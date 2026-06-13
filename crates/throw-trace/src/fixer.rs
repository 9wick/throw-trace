use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use throw_trace_core::{Diagnostic, ErrorType, FunctionId, PropagatedThrow};

pub fn fix_files(diagnostics: &[Diagnostic]) -> Result<usize> {
    let grouped = group_by_file(diagnostics);
    let mut fixed_count = 0;

    for (file_path, diags) in grouped {
        if apply_fixes(&file_path, &diags)? {
            fixed_count += 1;
        }
    }

    Ok(fixed_count)
}

fn group_by_file(diagnostics: &[Diagnostic]) -> HashMap<PathBuf, Vec<&Diagnostic>> {
    let mut grouped: HashMap<PathBuf, Vec<&Diagnostic>> = HashMap::new();
    for diag in diagnostics {
        grouped.entry(diag.function.file_path.clone()).or_default().push(diag);
    }
    grouped
}

enum Modification {
    Insert { line: usize, content: Vec<String> },
    AppendToJsdoc { jsdoc_end_line: usize, throws_lines: Vec<String> },
}

fn apply_fixes(file_path: &PathBuf, diagnostics: &[&Diagnostic]) -> Result<bool> {
    let raw = fs::read_to_string(file_path)?;
    // BOM は span のオフセットに含まれるため、剥がした分を行検索時に補正する
    let bom = if raw.starts_with('\u{FEFF}') { "\u{FEFF}" } else { "" };
    let source = &raw[bom.len()..];
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let had_trailing_newline = source.ends_with('\n');
    let lines: Vec<&str> = source.lines().collect();

    #[allow(clippy::cast_possible_truncation)]
    let bom_len = bom.len() as u32;
    let mut modifications = collect_modifications(source, &lines, bom_len, diagnostics);
    if modifications.is_empty() {
        return Ok(false);
    }

    modifications.sort_by(|a, b| {
        let line_a = match a {
            Modification::Insert { line, .. } => *line,
            Modification::AppendToJsdoc { jsdoc_end_line, .. } => *jsdoc_end_line,
        };
        let line_b = match b {
            Modification::Insert { line, .. } => *line,
            Modification::AppendToJsdoc { jsdoc_end_line, .. } => *jsdoc_end_line,
        };
        line_b.cmp(&line_a)
    });

    let mut result_lines: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();

    for modification in modifications {
        match modification {
            Modification::Insert { line, content } => {
                let indent = detect_indent(&result_lines, line);
                let formatted: Vec<String> =
                    content.iter().map(|l| format!("{indent}{l}")).collect();
                for (i, formatted_line) in formatted.into_iter().enumerate() {
                    result_lines.insert(line + i, formatted_line);
                }
            }
            Modification::AppendToJsdoc { jsdoc_end_line, throws_lines } => {
                let indent = detect_jsdoc_indent(&result_lines, jsdoc_end_line);
                let current_line = &result_lines[jsdoc_end_line];
                let closing_pos = current_line.rfind("*/").unwrap_or(current_line.len());
                let before_close = current_line[..closing_pos].trim_end();

                let mut new_lines: Vec<String> = Vec::new();
                if !before_close.is_empty() && before_close != "*" {
                    new_lines.push(before_close.to_string());
                }
                for throws_line in &throws_lines {
                    new_lines.push(format!("{indent} * {throws_line}"));
                }
                new_lines.push(format!("{indent} */"));

                result_lines.splice(jsdoc_end_line..=jsdoc_end_line, new_lines);
            }
        }
    }

    let mut output = String::with_capacity(raw.len());
    output.push_str(bom);
    output.push_str(&result_lines.join(newline));
    if had_trailing_newline {
        output.push_str(newline);
    }
    fs::write(file_path, output)?;
    Ok(true)
}

fn collect_modifications(
    source: &str,
    lines: &[&str],
    bom_len: u32,
    diagnostics: &[&Diagnostic],
) -> Vec<Modification> {
    let mut modifications = Vec::new();

    for diag in diagnostics {
        let func_line =
            find_function_line(source, diag.function.span.start.saturating_sub(bom_len));
        let Some(func_line) = func_line else {
            continue;
        };

        let throws_entries = generate_throws_entries(&diag.missing_throws);
        if throws_entries.is_empty() {
            continue;
        }

        if let Some(jsdoc_end_line) = find_jsdoc_end_line(lines, func_line) {
            modifications
                .push(Modification::AppendToJsdoc { jsdoc_end_line, throws_lines: throws_entries });
        } else {
            let mut comment = vec!["/**".to_string()];
            for entry in throws_entries {
                comment.push(format!(" * {entry}"));
            }
            comment.push(" */".to_string());
            modifications.push(Modification::Insert { line: func_line, content: comment });
        }
    }

    modifications
}

// オフセットより前にある改行を直接数えることで、CRLF/LF 混在でもずれない
fn find_function_line(source: &str, byte_offset: u32) -> Option<usize> {
    let offset = byte_offset as usize;
    if offset > source.len() {
        return None;
    }
    Some(source[..offset].matches('\n').count())
}

fn find_jsdoc_end_line(lines: &[&str], func_line: usize) -> Option<usize> {
    if func_line == 0 {
        return None;
    }

    let mut end_line = None;
    for i in (0..func_line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.ends_with("*/") {
            end_line = Some(i);
            break;
        }
        return None;
    }

    let end_line = end_line?;

    // JSDoc ブロックの内側だけを遡る。コメント行以外に到達した場合は、
    // 末尾ブロックコメント付きのコード行を JSDoc と誤認しているので打ち切る
    for i in (0..=end_line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("/**") {
            return Some(end_line);
        }
        if !trimmed.starts_with('*') {
            return None;
        }
    }
    None
}

fn detect_indent(lines: &[String], line_idx: usize) -> String {
    if line_idx >= lines.len() {
        return String::new();
    }

    let line = &lines[line_idx];
    let trimmed_len = line.trim_start().len();
    line[..line.len() - trimmed_len].to_string()
}

fn detect_jsdoc_indent(lines: &[String], jsdoc_end_line: usize) -> String {
    let line = &lines[jsdoc_end_line];
    let trimmed = line.trim_start();
    line[..line.len() - trimmed.len()].to_string()
}

fn generate_throws_entries(missing_throws: &[PropagatedThrow]) -> Vec<String> {
    missing_throws
        .iter()
        .map(|throw| {
            let type_name = match &throw.error_type {
                ErrorType::Named(name) | ErrorType::Rethrow(name) => name.as_str(),
                ErrorType::Unknown => "unknown",
            };
            let from_info = format_from_info(&throw.origin_function);
            format!("@throws {{{type_name}}} from {from_info}")
        })
        .collect()
}

fn format_from_info(origin_function: &FunctionId) -> String {
    let file_name =
        origin_function.file_path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");

    format!("{file_name}:{}", origin_function.name)
}

#[cfg(test)]
mod tests {
    use super::find_jsdoc_end_line;

    #[test]
    fn trailing_block_comment_on_code_line_is_not_jsdoc() {
        // 上方に無関係な JSDoc があっても、コード行を JSDoc 終端と誤認してはならない
        let lines = vec![
            "/** unrelated doc */",
            "function a() {}",
            "const x = 1; /* tail */",
            "function b() {}",
        ];
        assert_eq!(find_jsdoc_end_line(&lines, 3), None);
    }

    #[test]
    fn code_between_jsdoc_start_and_end_is_not_jsdoc() {
        let lines = vec!["/** doc for a */", "const a = 1; /* note */", "function b() {}"];
        assert_eq!(find_jsdoc_end_line(&lines, 2), None);
    }

    #[test]
    fn single_line_jsdoc_is_found() {
        let lines = vec!["/** doc */", "function a() {}"];
        assert_eq!(find_jsdoc_end_line(&lines, 1), Some(0));
    }

    #[test]
    fn multiline_jsdoc_is_found() {
        let lines = vec!["/**", " * doc", " */", "function a() {}"];
        assert_eq!(find_jsdoc_end_line(&lines, 3), Some(2));
    }

    #[test]
    fn plain_block_comment_is_not_jsdoc() {
        let lines = vec!["/* not doc */", "function a() {}"];
        assert_eq!(find_jsdoc_end_line(&lines, 1), None);
    }
}

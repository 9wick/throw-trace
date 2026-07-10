use crate::analyzer::ThrowContract;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use throw_trace_core::{ErrorType, FunctionId, PropagatedThrow};

pub fn fix_files(contracts: &[ThrowContract]) -> Result<usize> {
    let grouped = group_by_file(contracts);
    let mut fixed_count = 0;

    for (file_path, diags) in grouped {
        if apply_fixes(&file_path, &diags)? {
            fixed_count += 1;
        }
    }

    Ok(fixed_count)
}

fn group_by_file(contracts: &[ThrowContract]) -> HashMap<PathBuf, Vec<&ThrowContract>> {
    let mut grouped: HashMap<PathBuf, Vec<&ThrowContract>> = HashMap::new();
    for contract in contracts {
        grouped.entry(contract.function.file_path.clone()).or_default().push(contract);
    }
    grouped
}

enum Modification {
    Insert { line: usize, content: Vec<String> },
    Replace { start_line: usize, end_line: usize, content: Vec<String> },
}

fn apply_fixes(file_path: &PathBuf, contracts: &[&ThrowContract]) -> Result<bool> {
    let raw = fs::read_to_string(file_path)?;
    // BOM は span のオフセットに含まれるため、剥がした分を行検索時に補正する
    let bom = if raw.starts_with('\u{FEFF}') { "\u{FEFF}" } else { "" };
    let source = &raw[bom.len()..];
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let had_trailing_newline = source.ends_with('\n');
    let lines: Vec<&str> = source.lines().collect();

    #[allow(clippy::cast_possible_truncation)]
    let bom_len = bom.len() as u32;
    let mut modifications = collect_modifications(source, &lines, bom_len, contracts);
    if modifications.is_empty() {
        return Ok(false);
    }

    modifications.sort_by(|a, b| {
        let line_a = match a {
            Modification::Insert { line, .. } => *line,
            Modification::Replace { start_line, .. } => *start_line,
        };
        let line_b = match b {
            Modification::Insert { line, .. } => *line,
            Modification::Replace { start_line, .. } => *start_line,
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
            Modification::Replace { start_line, end_line, content } => {
                result_lines.splice(start_line..=end_line, content);
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
    contracts: &[&ThrowContract],
) -> Vec<Modification> {
    let mut modifications = Vec::new();

    for contract in contracts {
        let func_line =
            find_function_line(source, contract.function.span.start.saturating_sub(bom_len));
        let Some(func_line) = func_line else {
            continue;
        };

        let throws_entries = generate_throws_entries(&contract.required_throws);

        if let Some((start_line, end_line)) = find_jsdoc_range(lines, func_line) {
            let existing: Vec<String> =
                lines[start_line..=end_line].iter().map(|line| (*line).to_string()).collect();
            let content = sync_jsdoc(&existing, &throws_entries);
            if content != existing {
                modifications.push(Modification::Replace { start_line, end_line, content });
            }
        } else if !throws_entries.is_empty() {
            let mut comment = vec!["/**".to_string()];
            for entry in throws_entries {
                comment.push(format!(" * {}", entry.text));
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

fn find_jsdoc_range(lines: &[&str], func_line: usize) -> Option<(usize, usize)> {
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

    for i in (0..=end_line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("/**") {
            return Some((i, end_line));
        }
        if trimmed.starts_with("/*") && !trimmed.starts_with("/**") {
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

struct ThrowsEntry {
    type_name: String,
    text: String,
}

fn generate_throws_entries(required_throws: &[PropagatedThrow]) -> Vec<ThrowsEntry> {
    let mut seen = HashSet::new();
    required_throws
        .iter()
        .filter_map(|throw| {
            let type_name = match &throw.error_type {
                ErrorType::Named(name) | ErrorType::Rethrow(name) => name.as_str(),
                ErrorType::Unknown => "unknown",
            };
            if !seen.insert(type_name.to_string()) {
                return None;
            }
            let from_info = format_from_info(&throw.origin_function);
            Some(ThrowsEntry {
                type_name: type_name.to_string(),
                text: format!("@throws {{{type_name}}} from {from_info}"),
            })
        })
        .collect()
}

fn sync_jsdoc(existing: &[String], desired: &[ThrowsEntry]) -> Vec<String> {
    let desired_types: HashSet<&str> =
        desired.iter().map(|entry| entry.type_name.as_str()).collect();
    let mut matched = HashSet::new();
    let mut result = Vec::new();

    for line in existing {
        let Some(declaration) = parse_throws_declaration(line) else {
            result.push(line.clone());
            continue;
        };

        let is_desired = desired_types.contains(declaration.type_name.as_str());
        if declaration.manual {
            if is_desired {
                matched.insert(declaration.type_name.clone());
            }
            result.push(line.clone());
        } else if is_desired && matched.insert(declaration.type_name) {
            result.push(line.clone());
        }
    }

    let missing: Vec<&ThrowsEntry> =
        desired.iter().filter(|entry| !matched.contains(entry.type_name.as_str())).collect();

    if missing.is_empty() {
        return if has_jsdoc_content(&result) { result } else { Vec::new() };
    }

    if !has_jsdoc_content(&result) {
        let indent = existing.first().map_or("", |line| leading_whitespace(line));
        return generated_jsdoc(indent, &missing);
    }

    insert_missing_throws(result, &missing)
}

struct ParsedThrowsDeclaration {
    type_name: String,
    manual: bool,
}

fn parse_throws_declaration(line: &str) -> Option<ParsedThrowsDeclaration> {
    let tag_start = line.find("@throws")?;
    let after_tag = line[tag_start + "@throws".len()..].trim_start();
    let type_start = after_tag.strip_prefix('{')?;
    let type_end = type_start.find('}')?;
    let type_name = type_start[..type_end].trim();
    if type_name.is_empty() {
        return None;
    }

    let remainder = type_start[type_end + 1..]
        .trim()
        .strip_suffix("*/")
        .unwrap_or_else(|| type_start[type_end + 1..].trim())
        .trim();
    let manual = !remainder.is_empty() && !remainder.starts_with("from ");

    Some(ParsedThrowsDeclaration { type_name: type_name.to_string(), manual })
}

fn has_jsdoc_content(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        let mut content = line.trim();
        if let Some(rest) = content.strip_prefix("/**") {
            content = rest.trim();
        }
        if let Some(rest) = content.strip_suffix("*/") {
            content = rest.trim();
        }
        if let Some(rest) = content.strip_prefix('*') {
            content = rest.trim();
        }
        !content.is_empty()
    })
}

fn generated_jsdoc(indent: &str, entries: &[&ThrowsEntry]) -> Vec<String> {
    let mut result = vec![format!("{indent}/**")];
    result.extend(entries.iter().map(|entry| format!("{indent} * {}", entry.text)));
    result.push(format!("{indent} */"));
    result
}

fn insert_missing_throws(mut lines: Vec<String>, entries: &[&ThrowsEntry]) -> Vec<String> {
    let Some(closing_line) = lines.iter().rposition(|line| line.contains("*/")) else {
        return lines;
    };
    let indent = leading_whitespace(&lines[closing_line]).to_string();
    let closing_pos = lines[closing_line].rfind("*/").unwrap_or(lines[closing_line].len());
    let before_close = lines[closing_line][..closing_pos].trim_end().to_string();
    let mut replacement = Vec::new();

    if !before_close.trim().is_empty() && before_close.trim() != "*" {
        replacement.push(before_close);
    }
    replacement.extend(entries.iter().map(|entry| format!("{indent} * {}", entry.text)));
    replacement.push(format!("{indent} */"));
    lines.splice(closing_line..=closing_line, replacement);
    lines
}

fn leading_whitespace(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn format_from_info(origin_function: &FunctionId) -> String {
    let file_name =
        origin_function.file_path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");

    format!("{file_name}:{}", origin_function.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(type_name: &str) -> ThrowsEntry {
        ThrowsEntry {
            type_name: type_name.to_string(),
            text: format!("@throws {{{type_name}}} from input.ts:test"),
        }
    }

    #[test]
    fn stale_generated_throw_is_removed_without_dropping_jsdoc_description() {
        let existing = vec![
            "/**".to_string(),
            " * Performs the operation.".to_string(),
            " * @throws {StaleError} from old.ts:old".to_string(),
            " */".to_string(),
        ];

        let synced = sync_jsdoc(&existing, &[]);

        assert_eq!(
            synced,
            vec!["/**".to_string(), " * Performs the operation.".to_string(), " */".to_string()]
        );
    }

    #[test]
    fn missing_throw_is_added_to_existing_single_line_jsdoc() {
        let existing = vec!["/** Performs the operation. */".to_string()];

        let synced = sync_jsdoc(&existing, &[entry("AppError")]);

        assert_eq!(
            synced,
            vec![
                "/** Performs the operation.".to_string(),
                " * @throws {AppError} from input.ts:test".to_string(),
                " */".to_string()
            ]
        );
    }

    #[test]
    fn duplicate_generated_throws_are_collapsed_but_manual_description_is_preserved() {
        let existing = vec![
            "/**".to_string(),
            " * @throws {AppError}".to_string(),
            " * @throws {AppError} from input.ts:test".to_string(),
            " * @throws {ManualError} Documented by a maintainer.".to_string(),
            " */".to_string(),
        ];

        let synced = sync_jsdoc(&existing, &[entry("AppError")]);

        assert_eq!(
            synced,
            vec![
                "/**".to_string(),
                " * @throws {AppError}".to_string(),
                " * @throws {ManualError} Documented by a maintainer.".to_string(),
                " */".to_string()
            ]
        );
    }
}

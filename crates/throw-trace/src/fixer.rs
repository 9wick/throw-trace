use crate::analyzer::ThrowContract;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use throw_trace_core::{AnnotationTarget, ErrorType, FunctionId, PropagatedThrow};

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

struct Modification {
    start: usize,
    end: usize,
    content: String,
}

fn apply_fixes(file_path: &PathBuf, contracts: &[&ThrowContract]) -> Result<bool> {
    let source = fs::read_to_string(file_path)?;
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let mut modifications = collect_modifications(&source, newline, contracts);
    if modifications.is_empty() {
        return Ok(false);
    }

    modifications.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    let mut output = source;
    for edit in modifications {
        output.replace_range(edit.start..edit.end, &edit.content);
    }
    fs::write(file_path, output)?;
    Ok(true)
}

fn collect_modifications(
    source: &str,
    newline: &str,
    contracts: &[&ThrowContract],
) -> Vec<Modification> {
    let mut modifications = Vec::new();
    for contract in contracts {
        let anchor = match contract.annotation_target {
            AnnotationTarget::Declaration { start } => start,
            AnnotationTarget::Expression { span } => span.start,
        } as usize;
        let before = &source[..anchor];
        let line_start = before.rfind('\n').map_or(0, |pos| pos + 1);
        // Keep a BOM outside every replacement, including edits on the first line.
        let line_start = if line_start == 0 && source.starts_with('\u{FEFF}') {
            '\u{FEFF}'.len_utf8()
        } else {
            line_start
        };
        let line_end = source[line_start..].find('\n').map_or(source.len(), |pos| line_start + pos);
        let indent = leading_whitespace(&source[line_start..line_end]);
        let throws_entries = generate_throws_entries(&contract.required_throws);

        if let Some((start, end)) = preceding_jsdoc_range(source, anchor) {
            let comment_line =
                source[..start].rfind('\n').map_or(line_start.min(start), |pos| pos + 1);
            let standalone = source[comment_line..start].trim().is_empty();
            let edit_start = if standalone { comment_line } else { start };
            let existing: Vec<String> =
                source[edit_start..end].lines().map(str::to_owned).collect();
            let synced = sync_jsdoc(&existing, &throws_entries);
            let mut content = synced.join(newline);
            if content != source[edit_start..end] {
                let edit_end =
                    if content.is_empty() && standalone && source[end..].starts_with(newline) {
                        end + newline.len()
                    } else {
                        end
                    };
                if content.contains('\n') && !standalone {
                    if let AnnotationTarget::Expression { span } = contract.annotation_target {
                        content.insert(0, '(');
                        let end = span.end as usize;
                        modifications.push(Modification {
                            start: end,
                            end,
                            content: ")".to_string(),
                        });
                    }
                }
                modifications.push(Modification { start: edit_start, end: edit_end, content });
            }
        } else if !throws_entries.is_empty() {
            let entries: Vec<_> = throws_entries.iter().collect();
            let standalone = source[line_start..anchor].trim().is_empty();
            let start = if standalone { line_start } else { anchor };
            let mut comment = generated_jsdoc(indent, &entries);
            if !standalone {
                comment[0] = "/**".to_string();
            }
            let mut content = comment.join(newline);
            content.push_str(newline);
            if !standalone {
                content.push_str(indent);
                // A newline after `return` changes its meaning through ASI, even
                // inside a comment. Parentheses keep the annotated expression attached.
                if matches!(contract.annotation_target, AnnotationTarget::Expression { .. }) {
                    content.insert(0, '(');
                    if let AnnotationTarget::Expression { span } = contract.annotation_target {
                        let end = span.end as usize;
                        modifications.push(Modification {
                            start: end,
                            end,
                            content: ")".to_string(),
                        });
                    }
                }
            }
            modifications.push(Modification { start, end: start, content });
        }
    }
    modifications
}

fn preceding_jsdoc_range(source: &str, anchor: usize) -> Option<(usize, usize)> {
    let trimmed = source[..anchor].trim_end();
    if !trimmed.ends_with("*/") {
        return None;
    }
    Some((trimmed.rfind("/**")?, trimmed.len()))
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
                ErrorType::Unknown | ErrorType::CapturedRethrow { .. } => "unknown",
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

        let desired: Vec<_> = declaration
            .type_names
            .iter()
            .filter(|name| desired_types.contains(name.as_str()))
            .collect();
        if declaration.manual
            || (desired.len() == declaration.type_names.len()
                && desired.iter().any(|name| !matched.contains(name.as_str())))
        {
            matched.extend(desired.into_iter().cloned());
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
    type_names: Vec<String>,
    manual: bool,
}

fn parse_throws_declaration(line: &str) -> Option<ParsedThrowsDeclaration> {
    let declarations = throw_trace_ts::extract_throws_from_jsdoc(line);
    let manual = declarations.first()?.description.is_some();
    Some(ParsedThrowsDeclaration {
        type_names: declarations.into_iter().map(|decl| decl.type_name).collect(),
        manual,
    })
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
    let indent = lines.first().map_or("", |line| leading_whitespace(line)).to_string();
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

    #[test]
    fn missing_throw_uses_jsdoc_base_indent() {
        let existing = vec![
            "  /**".to_string(),
            "   * Performs the operation.".to_string(),
            "   */".to_string(),
        ];

        let synced = sync_jsdoc(&existing, &[entry("AppError")]);

        assert_eq!(
            synced,
            vec![
                "  /**".to_string(),
                "   * Performs the operation.".to_string(),
                "   * @throws {AppError} from input.ts:test".to_string(),
                "   */".to_string()
            ]
        );
    }
}

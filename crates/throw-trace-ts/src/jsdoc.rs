#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThrowsDeclaration {
    pub type_name: String,
    pub description: Option<String>,
    pub from: Option<FromInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromInfo {
    pub file_path: String,
    pub func_name: String,
}

/// Extract @throws declarations from a `JSDoc` comment string.
pub fn extract_throws_from_jsdoc(comment: &str) -> Vec<ThrowsDeclaration> {
    let mut results = Vec::new();

    for line in comment.lines() {
        let trimmed = line.trim().trim_start_matches('/').trim_start_matches('*').trim();

        if !trimmed.starts_with("@throws") {
            continue;
        }

        let rest = trimmed.strip_prefix("@throws").unwrap_or("").trim();

        if rest.starts_with('{') {
            if let Some((type_content, after)) = braced_type(rest) {
                let after_type = strip_jsdoc_end(after.trim());
                let (description, from) = parse_description_and_from(after_type);

                if let Some(types) = split_throw_types(type_content) {
                    for type_name in types {
                        results.push(ThrowsDeclaration {
                            type_name,
                            description: description.clone(),
                            from: from.clone(),
                        });
                    }
                }
            }
        } else {
            let parts: Vec<&str> = rest.splitn(2, char::is_whitespace).collect();
            if !parts.is_empty() && !parts[0].is_empty() {
                let type_name = parts[0].to_string();
                let after_type = strip_jsdoc_end(parts.get(1).map_or("", |s| s.trim()));
                let (description, from) = parse_description_and_from(after_type);
                results.push(ThrowsDeclaration { type_name, description, from });
            }
        }
    }

    results
}

fn braced_type(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0;
    let mut quote = None;
    let mut escaped = false;
    for (offset, ch) in text.char_indices() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&text[1..offset], &text[offset + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn split_throw_types(content: &str) -> Option<Vec<String>> {
    use oxc_ast::ast::{Statement, TSType};
    use oxc_span::GetSpan;

    let source = format!("type __Throws = {content};");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::ts()).parse();
    if !parsed.errors.is_empty() || parsed.program.body.len() != 1 {
        return None;
    }
    let Statement::TSTypeAliasDeclaration(alias) = parsed.program.body.first()? else {
        return None;
    };
    let types = if let TSType::TSUnionType(union) = &alias.type_annotation {
        union.types.iter().map(GetSpan::span).collect()
    } else {
        vec![alias.type_annotation.span()]
    };
    Some(
        types
            .into_iter()
            .map(|span| source[span.start as usize..span.end as usize].to_owned())
            .collect(),
    )
}

fn strip_jsdoc_end(text: &str) -> &str {
    text.trim_end_matches("*/").trim_end_matches('*').trim()
}

fn parse_description_and_from(text: &str) -> (Option<String>, Option<FromInfo>) {
    if text.is_empty() {
        return (None, None);
    }

    for (from_start, _) in text.match_indices("from ") {
        let after_from = text[from_start + 5..].trim();
        if let Some(from_info) = parse_from_info(after_from) {
            let before_from = text[..from_start].trim();
            let description =
                if before_from.is_empty() { None } else { Some(before_from.to_string()) };
            return (description, Some(from_info));
        }
    }

    (Some(text.to_string()), None)
}

fn parse_from_info(text: &str) -> Option<FromInfo> {
    let text = text.split_whitespace().next().unwrap_or(text);

    let colon_pos = text.rfind(':')?;
    if colon_pos == 0 || colon_pos == text.len() - 1 {
        return None;
    }

    let file_path = text[..colon_pos].to_string();
    let func_name = text[colon_pos + 1..].to_string();

    Some(FromInfo { file_path, func_name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_throws() {
        let result = extract_throws_from_jsdoc("/** @throws {Error} */");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].type_name, "Error");
        assert!(result[0].description.is_none());
        assert!(result[0].from.is_none());
    }

    #[test]
    fn parse_throws_with_description() {
        let result = extract_throws_from_jsdoc("/** @throws {Error} When something fails */");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].type_name, "Error");
        assert_eq!(result[0].description, Some("When something fails".to_string()));
        assert!(result[0].from.is_none());
    }

    #[test]
    fn parse_throws_with_from() {
        let result = extract_throws_from_jsdoc("/** @throws {DBError} from db.ts:query */");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].type_name, "DBError");
        assert!(result[0].description.is_none());
        let from = result[0].from.as_ref().unwrap();
        assert_eq!(from.file_path, "db.ts");
        assert_eq!(from.func_name, "query");
    }

    #[test]
    fn parse_throws_with_description_and_from() {
        let result =
            extract_throws_from_jsdoc("/** @throws {DBError} Database failure from db.ts:query */");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].type_name, "DBError");
        assert_eq!(result[0].description, Some("Database failure".to_string()));
        let from = result[0].from.as_ref().unwrap();
        assert_eq!(from.file_path, "db.ts");
        assert_eq!(from.func_name, "query");
    }

    #[test]
    fn parse_union_type_with_from() {
        let result =
            extract_throws_from_jsdoc("/** @throws {Error | DBError} from service.ts:save */");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].type_name, "Error");
        assert_eq!(result[1].type_name, "DBError");
        assert!(result[0].from.is_some());
        assert!(result[1].from.is_some());
    }

    #[test]
    fn parse_from_in_description_not_as_origin() {
        let result = extract_throws_from_jsdoc("/** @throws {Error} when reading from disk */");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].type_name, "Error");
        assert_eq!(result[0].description, Some("when reading from disk".to_string()));
        assert!(result[0].from.is_none());
    }

    #[test]
    fn nested_types_preserve_braces_and_internal_unions() {
        let result = extract_throws_from_jsdoc("/** @throws {Error | { code: 'a|b}'; detail: { value: string | number } } | null} details */");
        assert_eq!(
            result.iter().map(|decl| decl.type_name.as_str()).collect::<Vec<_>>(),
            ["Error", "{ code: 'a|b}'; detail: { value: string | number } }", "null"]
        );
        assert!(result.iter().all(|decl| decl.description.as_deref() == Some("details")));
    }

    #[test]
    fn function_and_generic_types_are_not_split_at_nested_unions() {
        let result = extract_throws_from_jsdoc(
            "/** @throws {(() => Error | null) | Promise<Error | null> | {}} */",
        );
        assert_eq!(
            result.iter().map(|decl| decl.type_name.as_str()).collect::<Vec<_>>(),
            ["(() => Error | null)", "Promise<Error | null>", "{}"]
        );
    }

    #[test]
    fn malformed_type_does_not_satisfy_a_throws_contract() {
        assert!(extract_throws_from_jsdoc("/** @throws {{ incomplete */").is_empty());
        assert!(extract_throws_from_jsdoc("/** @throws {Error | } */").is_empty());
    }
}

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
            if let Some(end_brace) = rest.find('}') {
                let type_content = &rest[1..end_brace];
                let after_type = strip_jsdoc_end(rest[end_brace + 1..].trim());
                let (description, from) = parse_description_and_from(after_type);

                for type_part in type_content.split('|') {
                    let type_name = type_part.trim().to_string();
                    if !type_name.is_empty() {
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
}

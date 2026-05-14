/// Extract @throws declarations from a `JSDoc` comment string.
/// Returns Vec of (`type_name`, `optional_description`).
pub fn extract_throws_from_jsdoc(comment: &str) -> Vec<(String, Option<String>)> {
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
                let description = rest[end_brace + 1..].trim();
                let desc =
                    if description.is_empty() { None } else { Some(description.to_string()) };

                for type_part in type_content.split('|') {
                    let type_name = type_part.trim().to_string();
                    if !type_name.is_empty() {
                        results.push((type_name, desc.clone()));
                    }
                }
            }
        } else {
            let parts: Vec<&str> = rest.splitn(2, char::is_whitespace).collect();
            if !parts.is_empty() && !parts[0].is_empty() {
                let type_name = parts[0].to_string();
                let description = parts.get(1).map(|s| s.trim().to_string());
                results.push((type_name, description));
            }
        }
    }

    results
}

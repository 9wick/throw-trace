//! TypeScript adapter for throw-trace (oxc-based).

mod extract;
mod jsdoc;
mod parser;
mod throw_analyzer;
mod try_catch;
pub mod tsserver;

pub use extract::{extract_all, extract_functions, ExtractionResult};
pub use jsdoc::extract_throws_from_jsdoc;
pub use parser::parse_source;
pub use throw_analyzer::{analyze_throw_expr, analyze_throw_expr_with_catch_params};
pub use try_catch::extract_try_catch_blocks;
pub use tsserver::{byte_offset_to_line_col, TsServer, TsServerError, TsServerTypeResolver};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use throw_trace_core::ErrorType;

    #[test]
    fn parse_source_returns_program() {
        let source = "function foo() { return 1; }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_handles_typescript() {
        let source = "function foo(x: number): string { return x.toString(); }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_reports_syntax_error() {
        let source = "function foo( { }";
        let result = parse_source(source);
        assert!(result.is_err());
    }

    #[test]
    fn extract_functions_finds_function_declaration() {
        let source = "function foo() { }\nfunction bar() { }";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 2);
        assert_eq!(sigs[0].id.name.as_str(), "foo");
        assert_eq!(sigs[1].id.name.as_str(), "bar");
    }

    #[test]
    fn extract_functions_finds_arrow_function() {
        let source = "const add = (a: number, b: number) => a + b;";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].id.name.as_str(), "add");
    }

    #[test]
    fn extract_functions_finds_async_function() {
        let source = "async function fetchData() { }";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].is_async);
    }

    #[test]
    fn extract_throws_single() {
        let comment = "/**\n * @throws {ValidationError} When input is invalid\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].0, "ValidationError");
        assert_eq!(throws[0].1.as_deref(), Some("When input is invalid"));
    }

    #[test]
    fn extract_throws_multiple() {
        let comment =
            "/**\n * @throws {ValidationError}\n * @throws {NetworkError} Connection failed\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].0, "ValidationError");
        assert_eq!(throws[1].0, "NetworkError");
    }

    #[test]
    fn extract_throws_no_braces() {
        let comment = "/** @throws Error when something fails */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].0, "Error");
    }

    #[test]
    fn extract_throws_empty() {
        let comment = "/** This is a description */";
        let throws = extract_throws_from_jsdoc(comment);
        assert!(throws.is_empty());
    }

    #[test]
    fn extract_throws_union_type() {
        let comment = "/**\n * @throws {ValidationError | NetworkError} When something fails\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].0, "ValidationError");
        assert_eq!(throws[0].1.as_deref(), Some("When something fails"));
        assert_eq!(throws[1].0, "NetworkError");
        assert_eq!(throws[1].1.as_deref(), Some("When something fails"));
    }

    #[test]
    fn extract_throws_union_type_three() {
        let comment = "/**\n * @throws {A | B | C}\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 3);
        assert_eq!(throws[0].0, "A");
        assert_eq!(throws[1].0, "B");
        assert_eq!(throws[2].0, "C");
    }

    #[test]
    fn analyze_throw_new_error() {
        let source = "throw new ValidationError('msg')";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Named("ValidationError".into()));
    }

    #[test]
    fn analyze_throw_new_error_simple() {
        let source = "throw new Error('msg')";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Named("Error".into()));
    }

    #[test]
    fn analyze_throw_literal() {
        let source = "throw 'error'";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Unknown);
    }

    #[test]
    fn analyze_throw_variable_is_unknown() {
        // throw variable (not catch param) should be Unknown
        let source = "throw err";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Unknown);
    }

    #[test]
    fn analyze_throw_catch_param_is_rethrow() {
        // throw catch param should be Rethrow
        let source = "throw e";
        let catch_params = vec!["e".to_string()];
        let result = analyze_throw_expr_with_catch_params(source, &catch_params);
        assert_eq!(result, ErrorType::Rethrow("e".into()));
    }

    #[test]
    fn analyze_throw_non_catch_param_is_unknown() {
        // throw variable that is NOT a catch param should be Unknown
        let source = "throw err";
        let catch_params = vec!["e".to_string()];
        let result = analyze_throw_expr_with_catch_params(source, &catch_params);
        assert_eq!(result, ErrorType::Unknown);
    }

    #[test]
    fn extract_try_catch_simple() {
        let source = r#"
try {
    validate();
} catch (e) {
    console.log(e);
}
"#;
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].catch_span.is_some());
    }

    #[test]
    fn extract_try_catch_with_instanceof() {
        let source = r#"
try {
    validate();
} catch (e) {
    if (e instanceof ValidationError) {
        return;
    }
    throw e;
}
"#;
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].caught_types.len(), 1);
        assert_eq!(blocks[0].caught_types[0].as_str(), "ValidationError");
    }

    #[test]
    fn extract_try_catch_no_catch() {
        let source = r#"
try {
    validate();
} finally {
    cleanup();
}
"#;
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].catch_span.is_none());
    }

    #[test]
    fn rethrow_in_catch_is_rethrow() {
        let source = r#"
function foo() {
    try {
        bar();
    } catch (e) {
        throw e;
    }
}
"#;
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].direct_throws.len(), 1);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Rethrow("e".into()));
    }

    #[test]
    fn throw_variable_outside_catch_is_unknown() {
        let source = r#"
function foo() {
    const err = getError();
    throw err;
}
"#;
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].direct_throws.len(), 1);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Unknown);
    }

    #[test]
    fn throw_typed_variable_uses_annotation() {
        let source = r#"
function foo() {
    const err: SomeError = getError();
    throw err;
}
"#;
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].direct_throws.len(), 1);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Named("SomeError".into()));
    }

    #[test]
    fn throw_union_typed_variable_extracts_all() {
        let source = r#"
function foo() {
    const err: ErrorA | ErrorB = getError();
    throw err;
}
"#;
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        // Union type creates multiple throw sites
        assert_eq!(sigs[0].direct_throws.len(), 2);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Named("ErrorA".into()));
        assert_eq!(sigs[0].direct_throws[1].error_type, ErrorType::Named("ErrorB".into()));
    }

    #[test]
    fn extract_interface_with_method_throws() {
        let source = r#"
interface UserRepository {
    /**
     * @throws {NotFoundError}
     */
    findById(id: string): User;
}
"#;
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.method_signatures.len(), 1);
        assert_eq!(result.method_signatures[0].method_name.as_str(), "findById");
        assert_eq!(result.method_signatures[0].declared_throws.len(), 1);
        assert_eq!(
            result.method_signatures[0].declared_throws[0].error_type.as_str(),
            "NotFoundError"
        );
    }

    #[test]
    fn extract_class_implements_interface() {
        let source = r#"
interface Repository {}
class UserRepository implements Repository {}
"#;
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.type_relations.len(), 1);
        assert_eq!(result.type_relations[0].child.name.as_str(), "UserRepository");
        assert_eq!(result.type_relations[0].parent.name.as_str(), "Repository");
        assert_eq!(result.type_relations[0].kind, throw_trace_core::RelationKind::Implements);
    }

    #[test]
    fn extract_class_extends_class() {
        let source = r#"
class BaseService {}
class UserService extends BaseService {}
"#;
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.type_relations.len(), 1);
        assert_eq!(result.type_relations[0].child.name.as_str(), "UserService");
        assert_eq!(result.type_relations[0].parent.name.as_str(), "BaseService");
        assert_eq!(result.type_relations[0].kind, throw_trace_core::RelationKind::Extends);
    }

    #[test]
    fn extract_abstract_class_method() {
        let source = r#"
abstract class BaseRepository {
    /**
     * @throws {NotFoundError}
     */
    abstract findById(id: string): User;
}
"#;
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.method_signatures.len(), 1);
        assert_eq!(result.method_signatures[0].method_name.as_str(), "findById");
        assert!(result.method_signatures[0].is_abstract);
        assert_eq!(result.method_signatures[0].declared_throws.len(), 1);
    }
}

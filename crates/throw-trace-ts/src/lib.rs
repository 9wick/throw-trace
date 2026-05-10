//! TypeScript adapter for throw-trace (oxc-based).

mod extract;
mod jsdoc;
mod parser;
mod throw_analyzer;

pub use extract::extract_functions;
pub use jsdoc::extract_throws_from_jsdoc;
pub use parser::parse_source;
pub use throw_analyzer::analyze_throw_expr;

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
        let comment = "/**\n * @throws {ValidationError}\n * @throws {NetworkError} Connection failed\n */";
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
    fn analyze_throw_variable() {
        let source = "throw err";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Unknown);
    }
}

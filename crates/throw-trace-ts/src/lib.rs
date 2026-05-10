//! TypeScript adapter for throw-trace (oxc-based).

mod extract;
mod parser;

pub use extract::extract_functions;
pub use parser::parse_source;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}

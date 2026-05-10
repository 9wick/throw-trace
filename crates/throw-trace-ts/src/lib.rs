//! TypeScript adapter for throw-trace (oxc-based).

mod parser;

pub use parser::parse_source;

#[cfg(test)]
mod tests {
    use super::*;

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
}

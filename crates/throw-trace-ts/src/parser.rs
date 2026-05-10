use oxc_allocator::Allocator;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Syntax error: {0}")]
    SyntaxError(String),
}

pub fn parse_source(source: &str) -> Result<(), ParseError> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return: ParserReturn = Parser::new(&allocator, source, source_type).parse();

    if parser_return.errors.is_empty() {
        Ok(())
    } else {
        let msg = parser_return
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        Err(ParseError::SyntaxError(msg))
    }
}

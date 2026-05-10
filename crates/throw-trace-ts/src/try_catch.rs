use compact_str::CompactString;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use throw_trace_core::{Span, TryCatchBlock};

struct TryCatchExtractor {
    blocks: Vec<TryCatchBlock>,
}

impl TryCatchExtractor {
    fn new() -> Self {
        Self { blocks: Vec::new() }
    }
}

impl<'a> Visit<'a> for TryCatchExtractor {
    fn visit_try_statement(&mut self, stmt: &oxc_ast::ast::TryStatement<'a>) {
        let try_span = Span { start: stmt.block.span.start, end: stmt.block.span.end };

        let (catch_span, caught_types) = if let Some(handler) = &stmt.handler {
            let span = Some(Span { start: handler.span.start, end: handler.span.end });
            let types = extract_instanceof_checks(&handler.body);
            (span, types)
        } else {
            (None, Vec::new())
        };

        self.blocks.push(TryCatchBlock { try_span, catch_span, caught_types });

        walk::walk_try_statement(self, stmt);
    }
}

fn extract_instanceof_checks(block: &oxc_ast::ast::BlockStatement) -> Vec<CompactString> {
    let mut types = Vec::new();

    for stmt in &block.body {
        if let Statement::IfStatement(if_stmt) = stmt {
            if let Expression::BinaryExpression(bin) = &if_stmt.test {
                if bin.operator == oxc_ast::ast::BinaryOperator::Instanceof {
                    if let Expression::Identifier(id) = &bin.right {
                        types.push(id.name.as_str().into());
                    }
                }
            }
        }
    }

    types
}

pub fn extract_try_catch_blocks(source: &str) -> Vec<TryCatchBlock> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    // Only bail on catastrophic parse failure; recoverable errors still produce a usable AST.
    if parser_return.panicked {
        return Vec::new();
    }

    let mut extractor = TryCatchExtractor::new();
    extractor.visit_program(&parser_return.program);

    extractor.blocks
}

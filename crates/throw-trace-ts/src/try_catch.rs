use oxc_allocator::Allocator;
use oxc_ast::ast::BindingPatternKind;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use throw_trace_core::{Span, TryCatchBlock};

use crate::extract::extract_instanceof_types;

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
            let catch_param = handler.param.as_ref().and_then(|p| {
                if let BindingPatternKind::BindingIdentifier(id) = &p.pattern.kind {
                    Some(id.name.as_str())
                } else {
                    None
                }
            });
            let types = extract_instanceof_types(&handler.body, catch_param);
            (span, types)
        } else {
            (None, Vec::new())
        };

        self.blocks.push(TryCatchBlock { try_span, catch_span, caught_types });

        walk::walk_try_statement(self, stmt);
    }
}

pub fn extract_try_catch_blocks(source: &str) -> Vec<TryCatchBlock> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    if parser_return.panicked {
        return Vec::new();
    }

    let mut extractor = TryCatchExtractor::new();
    extractor.visit_program(&parser_return.program);

    extractor.blocks
}

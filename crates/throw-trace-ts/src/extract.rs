use oxc_allocator::Allocator;
use oxc_ast::ast::{BindingPatternKind, Function};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;
use std::path::Path;
use throw_trace_core::{FunctionId, FunctionSignature, Span};

use crate::parser::ParseError;

struct FunctionExtractor<'a> {
    file_path: &'a Path,
    signatures: Vec<FunctionSignature>,
}

impl<'a> FunctionExtractor<'a> {
    fn new(file_path: &'a Path) -> Self {
        Self {
            file_path,
            signatures: Vec::new(),
        }
    }

    fn add_function(&mut self, name: &str, span: oxc_span::Span, is_async: bool) {
        let id = FunctionId::new(
            self.file_path.to_path_buf(),
            name,
            Span {
                start: span.start,
                end: span.end,
            },
        );
        self.signatures.push(FunctionSignature {
            id,
            declared_throws: Vec::new(),
            direct_throws: Vec::new(),
            calls: Vec::new(),
            try_catch_blocks: Vec::new(),
            is_async,
        });
    }
}

impl<'a> Visit<'a> for FunctionExtractor<'a> {
    fn visit_function(&mut self, func: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &func.id {
            self.add_function(id.name.as_str(), func.span, func.r#async);
        }
        walk::walk_function(self, func, flags);
    }

    fn visit_variable_declaration(&mut self, decl: &oxc_ast::ast::VariableDeclaration<'a>) {
        for declarator in &decl.declarations {
            if let Some(init) = &declarator.init {
                if let oxc_ast::ast::Expression::ArrowFunctionExpression(arrow) = init {
                    if let BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind {
                        self.add_function(id.name.as_str(), arrow.span, arrow.r#async);
                    }
                }
            }
        }
        walk::walk_variable_declaration(self, decl);
    }
}

pub fn extract_functions(
    source: &str,
    file_path: &Path,
) -> Result<Vec<FunctionSignature>, ParseError> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    if !parser_return.errors.is_empty() {
        let msg = parser_return
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ParseError::SyntaxError(msg));
    }

    let mut extractor = FunctionExtractor::new(file_path);
    extractor.visit_program(&parser_return.program);

    Ok(extractor.signatures)
}

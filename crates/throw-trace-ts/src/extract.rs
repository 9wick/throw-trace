use compact_str::CompactString;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPatternKind, CallExpression, Expression, Function, Statement, ThrowStatement,
    TryStatement,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;
use std::path::Path;
use throw_trace_core::{
    CallSite, DeclaredThrow, FunctionId, FunctionSignature, Span, ThrowSite, TryCatchBlock,
};

use crate::jsdoc::extract_throws_from_jsdoc;
use crate::parser::ParseError;
use crate::throw_analyzer::analyze_throw_expr;

struct FunctionExtractor<'a> {
    source: &'a str,
    file_path: &'a Path,
    signatures: Vec<FunctionSignature>,
    // Stack of indices into `signatures` representing the current function scope chain.
    // The last element is the innermost (current) function.
    scope_stack: Vec<usize>,
}

impl<'a> FunctionExtractor<'a> {
    fn new(source: &'a str, file_path: &'a Path) -> Self {
        Self { source, file_path, signatures: Vec::new(), scope_stack: Vec::new() }
    }

    fn begin_function(
        &mut self,
        name: &str,
        span: oxc_span::Span,
        is_async: bool,
        preceding_comment: Option<&str>,
    ) -> usize {
        let id = FunctionId::new(
            self.file_path.to_path_buf(),
            name,
            Span { start: span.start, end: span.end },
        );

        let declared_throws =
            preceding_comment.map(|c| parse_declared_throws(c, span)).unwrap_or_default();

        let idx = self.signatures.len();
        self.signatures.push(FunctionSignature {
            id,
            declared_throws,
            direct_throws: Vec::new(),
            calls: Vec::new(),
            try_catch_blocks: Vec::new(),
            is_async,
        });
        self.scope_stack.push(idx);
        idx
    }

    fn end_function(&mut self) {
        self.scope_stack.pop();
    }

    fn current_sig_mut(&mut self) -> Option<&mut FunctionSignature> {
        self.scope_stack.last().copied().and_then(|idx| self.signatures.get_mut(idx))
    }

    fn add_throw(&mut self, throw_stmt: &ThrowStatement<'_>) {
        // Extract snippet before taking the mutable borrow to avoid borrow conflict.
        let snippet =
            self.source[throw_stmt.span.start as usize..throw_stmt.span.end as usize].to_owned();
        let error_type = analyze_throw_expr(&snippet);
        let Some(sig) = self.current_sig_mut() else {
            return;
        };
        sig.direct_throws.push(ThrowSite {
            location: Span { start: throw_stmt.span.start, end: throw_stmt.span.end },
            error_type,
        });
    }

    fn add_call(&mut self, call_expr: &CallExpression<'_>) {
        let callee_name = extract_callee_name(&call_expr.callee);
        let Some(name) = callee_name else { return };
        let Some(sig) = self.current_sig_mut() else {
            return;
        };
        sig.calls.push(CallSite {
            callee_name: name,
            location: Span { start: call_expr.span.start, end: call_expr.span.end },
        });
    }

    fn add_try_catch(&mut self, stmt: &TryStatement<'_>) {
        let try_span = Span { start: stmt.block.span.start, end: stmt.block.span.end };

        let (catch_span, caught_types) = if let Some(handler) = &stmt.handler {
            let span = Some(Span { start: handler.span.start, end: handler.span.end });
            let types = extract_instanceof_types(&handler.body);
            (span, types)
        } else {
            (None, Vec::new())
        };

        let Some(sig) = self.current_sig_mut() else {
            return;
        };
        sig.try_catch_blocks.push(TryCatchBlock { try_span, catch_span, caught_types });
    }
}

fn extract_callee_name(expr: &Expression<'_>) -> Option<CompactString> {
    match expr {
        Expression::Identifier(id) => Some(id.name.as_str().into()),
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str().into()),
        _ => None,
    }
}

fn extract_instanceof_types(block: &oxc_ast::ast::BlockStatement) -> Vec<CompactString> {
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

fn parse_declared_throws(comment: &str, _span: oxc_span::Span) -> Vec<DeclaredThrow> {
    extract_throws_from_jsdoc(comment)
        .into_iter()
        .map(|(type_name, description)| DeclaredThrow {
            error_type: type_name.into(),
            description: description.map(Into::into),
            // Span is not tracked precisely for JSDoc; use zeroes as placeholder.
            span: Span { start: 0, end: 0 },
        })
        .collect()
}

impl<'a> Visit<'a> for FunctionExtractor<'a> {
    fn visit_function(&mut self, func: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &func.id {
            // Look for a preceding JSDoc comment via the raw source
            let comment = preceding_jsdoc(self.source, func.span.start);
            self.begin_function(id.name.as_str(), func.span, func.r#async, comment.as_deref());
            walk::walk_function(self, func, flags);
            self.end_function();
        } else {
            // Anonymous function: still walk but don't create a signature
            walk::walk_function(self, func, flags);
        }
    }

    fn visit_variable_declaration(&mut self, decl: &oxc_ast::ast::VariableDeclaration<'a>) {
        for declarator in &decl.declarations {
            if let Some(Expression::ArrowFunctionExpression(arrow)) = &declarator.init {
                if let BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind {
                    let comment = preceding_jsdoc(self.source, decl.span.start);
                    self.begin_function(
                        id.name.as_str(),
                        arrow.span,
                        arrow.r#async,
                        comment.as_deref(),
                    );
                    walk::walk_arrow_function_expression(self, arrow);
                    self.end_function();
                } else {
                    walk::walk_arrow_function_expression(self, arrow);
                }
                // Skip the default variable walk for this declarator to avoid re-entering
                continue;
            }
            walk::walk_variable_declarator(self, declarator);
        }
    }

    fn visit_throw_statement(&mut self, stmt: &ThrowStatement<'a>) {
        self.add_throw(stmt);
        walk::walk_throw_statement(self, stmt);
    }

    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        self.add_call(expr);
        walk::walk_call_expression(self, expr);
    }

    fn visit_try_statement(&mut self, stmt: &TryStatement<'a>) {
        self.add_try_catch(stmt);
        walk::walk_try_statement(self, stmt);
    }
}

/// Find the `JSDoc` comment immediately preceding the given byte offset in source.
/// Returns the raw comment text if found.
fn preceding_jsdoc(source: &str, start: u32) -> Option<String> {
    let before = &source[..start as usize];
    let trimmed = before.trim_end();

    if !trimmed.ends_with("*/") {
        return None;
    }

    let end = trimmed.len();
    let comment_end = end; // position of '*/'
    let comment_start = trimmed.rfind("/**")?;

    // Make sure there's only whitespace between the comment end and the function start
    let between = &before[comment_end..];
    if !between.trim().is_empty() {
        return None;
    }

    Some(trimmed[comment_start..].to_string())
}

pub fn extract_functions(
    source: &str,
    file_path: &Path,
) -> Result<Vec<FunctionSignature>, ParseError> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    if !parser_return.errors.is_empty() {
        let msg =
            parser_return.errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
        return Err(ParseError::SyntaxError(msg));
    }

    let mut extractor = FunctionExtractor::new(source, file_path);
    extractor.visit_program(&parser_return.program);

    Ok(extractor.signatures)
}

use compact_str::CompactString;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPatternKind, CallExpression, Class, Expression, Function, MethodDefinition, Statement,
    ThrowStatement, TryStatement, TSInterfaceDeclaration, TSType,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;
use std::collections::HashMap;
use std::path::Path;
use throw_trace_core::{
    CallSite, DeclaredThrow, ErrorType, FunctionId, FunctionSignature, MethodSignature,
    RelationKind, Span, ThrowSite, TryCatchBlock, TypeId, TypeRelation,
};

use crate::jsdoc::extract_throws_from_jsdoc;
use crate::parser::ParseError;
use crate::throw_analyzer::analyze_throw_expr_with_catch_params;

pub struct ExtractionResult {
    pub signatures: Vec<FunctionSignature>,
    pub method_signatures: Vec<MethodSignature>,
    pub type_relations: Vec<TypeRelation>,
}

struct FunctionExtractor<'a> {
    source: &'a str,
    file_path: &'a Path,
    signatures: Vec<FunctionSignature>,
    method_signatures: Vec<MethodSignature>,
    type_relations: Vec<TypeRelation>,
    // Stack of indices into `signatures` representing the current function scope chain.
    // The last element is the innermost (current) function.
    scope_stack: Vec<usize>,
    // Stack of catch parameter names (e.g., "e" from "catch (e)")
    catch_param_stack: Vec<String>,
    // Variable name -> type annotation (supports union types as Vec)
    variable_types: HashMap<String, Vec<String>>,
    // Current class context for method extraction
    current_class: Option<TypeId>,
}

impl<'a> FunctionExtractor<'a> {
    fn new(source: &'a str, file_path: &'a Path) -> Self {
        Self {
            source,
            file_path,
            signatures: Vec::new(),
            method_signatures: Vec::new(),
            type_relations: Vec::new(),
            scope_stack: Vec::new(),
            catch_param_stack: Vec::new(),
            variable_types: HashMap::new(),
            current_class: None,
        }
    }

    fn begin_function(
        &mut self,
        name: &str,
        name_span: oxc_span::Span,
        func_span: oxc_span::Span,
        is_async: bool,
        preceding_comment: Option<&str>,
    ) -> usize {
        let id = FunctionId::new(
            self.file_path.to_path_buf(),
            name,
            Span { start: func_span.start, end: func_span.end },
        );

        let declared_throws =
            preceding_comment.map(|c| parse_declared_throws(c, func_span)).unwrap_or_default();

        let class_name = self.current_class.as_ref().map(|c| c.name.clone());

        let idx = self.signatures.len();
        self.signatures.push(FunctionSignature {
            id,
            name_span: Span { start: name_span.start, end: name_span.end },
            declared_throws,
            direct_throws: Vec::new(),
            calls: Vec::new(),
            try_catch_blocks: Vec::new(),
            is_async,
            class_name,
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
        let location = Span { start: throw_stmt.span.start, end: throw_stmt.span.end };

        // Check if throwing a typed variable
        if let Expression::Identifier(id) = &throw_stmt.argument {
            let var_name = id.name.as_str();
            if let Some(types) = self.variable_types.get(var_name).cloned() {
                let Some(sig) = self.current_sig_mut() else {
                    return;
                };
                for type_name in types {
                    sig.direct_throws.push(ThrowSite {
                        location,
                        error_type: ErrorType::Named(type_name.as_str().into()),
                    });
                }
                return;
            }
        }

        // Fallback to expression analysis
        let snippet =
            self.source[throw_stmt.span.start as usize..throw_stmt.span.end as usize].to_owned();
        let error_type = analyze_throw_expr_with_catch_params(&snippet, &self.catch_param_stack);
        let Some(sig) = self.current_sig_mut() else {
            return;
        };
        sig.direct_throws.push(ThrowSite { location, error_type });
    }

    fn record_variable_type(&mut self, name: &str, ts_type: &TSType<'_>) {
        let types = extract_type_names(ts_type);
        if !types.is_empty() {
            self.variable_types.insert(name.to_string(), types);
        }
    }

    fn add_call(&mut self, call_expr: &CallExpression<'_>) {
        let (callee_name, callee_span) = extract_callee_info(&call_expr.callee);
        let Some(name) = callee_name else { return };
        let Some(span) = callee_span else { return };
        let Some(sig) = self.current_sig_mut() else {
            return;
        };
        sig.calls.push(CallSite {
            callee_name: name,
            callee_span: Span { start: span.start, end: span.end },
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

    fn extract_interface(&mut self, iface: &TSInterfaceDeclaration<'_>) {
        let type_id = TypeId::new(
            self.file_path.to_path_buf(),
            iface.id.name.as_str(),
            Span { start: iface.span.start, end: iface.span.end },
        );

        // Extract extends clauses
        for heritage in &iface.extends {
            if let Expression::Identifier(id) = &heritage.expression {
                let parent_id = TypeId::new(
                    self.file_path.to_path_buf(),
                    id.name.as_str(),
                    Span { start: id.span.start, end: id.span.end },
                );
                self.type_relations.push(TypeRelation {
                    child: type_id.clone(),
                    parent: parent_id,
                    kind: RelationKind::Extends,
                });
            }
        }

        // Extract method signatures
        for sig in &iface.body.body {
            if let oxc_ast::ast::TSSignature::TSMethodSignature(method) = sig {
                let method_name = match &method.key {
                    oxc_ast::ast::PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                    _ => continue,
                };

                let comment = preceding_jsdoc(self.source, method.span.start);
                let declared_throws = comment
                    .map(|c| parse_declared_throws(&c, method.span))
                    .unwrap_or_default();

                self.method_signatures.push(MethodSignature {
                    type_id: type_id.clone(),
                    method_name: method_name.into(),
                    method_span: Span { start: method.span.start, end: method.span.end },
                    declared_throws,
                    is_abstract: false,
                });
            }
        }
    }

    fn extract_class(&mut self, class: &Class<'_>) {
        let class_name = match &class.id {
            Some(id) => id.name.as_str(),
            None => return,
        };

        let type_id = TypeId::new(
            self.file_path.to_path_buf(),
            class_name,
            Span { start: class.span.start, end: class.span.end },
        );

        // Extract extends clause
        if let Some(super_class) = &class.super_class {
            if let Expression::Identifier(id) = super_class {
                let parent_id = TypeId::new(
                    self.file_path.to_path_buf(),
                    id.name.as_str(),
                    Span { start: id.span.start, end: id.span.end },
                );
                self.type_relations.push(TypeRelation {
                    child: type_id.clone(),
                    parent: parent_id,
                    kind: RelationKind::Extends,
                });
            }
        }

        // Extract implements clauses
        for impl_clause in &class.implements {
            if let oxc_ast::ast::TSTypeName::IdentifierReference(id) = &impl_clause.expression {
                let parent_id = TypeId::new(
                    self.file_path.to_path_buf(),
                    id.name.as_str(),
                    Span { start: id.span.start, end: id.span.end },
                );
                self.type_relations.push(TypeRelation {
                    child: type_id.clone(),
                    parent: parent_id,
                    kind: RelationKind::Implements,
                });
            }
        }

        // Extract abstract method signatures from abstract classes
        if class.r#abstract {
            for element in &class.body.body {
                if let oxc_ast::ast::ClassElement::MethodDefinition(method) = element {
                    if method.r#type == oxc_ast::ast::MethodDefinitionType::TSAbstractMethodDefinition
                    {
                        let method_name = match &method.key {
                            oxc_ast::ast::PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                            _ => continue,
                        };

                        let comment = preceding_jsdoc(self.source, method.span.start);
                        let declared_throws = comment
                            .map(|c| parse_declared_throws(&c, method.span))
                            .unwrap_or_default();

                        self.method_signatures.push(MethodSignature {
                            type_id: type_id.clone(),
                            method_name: method_name.into(),
                            method_span: Span { start: method.span.start, end: method.span.end },
                            declared_throws,
                            is_abstract: true,
                        });
                    }
                }
            }
        }

        self.current_class = Some(type_id);
    }
}

fn extract_callee_info(expr: &Expression<'_>) -> (Option<CompactString>, Option<oxc_span::Span>) {
    match expr {
        Expression::Identifier(id) => (Some(id.name.as_str().into()), Some(id.span)),
        _ => (None, None),
    }
}

fn extract_type_names(ts_type: &TSType<'_>) -> Vec<String> {
    match ts_type {
        TSType::TSTypeReference(type_ref) => {
            if let oxc_ast::ast::TSTypeName::IdentifierReference(id) = &type_ref.type_name {
                vec![id.name.as_str().to_string()]
            } else {
                vec![]
            }
        }
        TSType::TSUnionType(union) => {
            union.types.iter().flat_map(|t| extract_type_names(t)).collect()
        }
        _ => vec![],
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
            self.begin_function(id.name.as_str(), id.span, func.span, func.r#async, comment.as_deref());
            walk::walk_function(self, func, flags);
            self.end_function();
        } else {
            // Anonymous function: still walk but don't create a signature
            walk::walk_function(self, func, flags);
        }
    }

    fn visit_export_named_declaration(
        &mut self,
        decl: &oxc_ast::ast::ExportNamedDeclaration<'a>,
    ) {
        match &decl.declaration {
            Some(oxc_ast::ast::Declaration::VariableDeclaration(var_decl)) => {
                for declarator in &var_decl.declarations {
                    if let Some(Expression::ArrowFunctionExpression(arrow)) = &declarator.init {
                        if let BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind {
                            let comment = preceding_jsdoc(self.source, decl.span.start);
                            self.begin_function(
                                id.name.as_str(),
                                id.span,
                                arrow.span,
                                arrow.r#async,
                                comment.as_deref(),
                            );
                            walk::walk_arrow_function_expression(self, arrow);
                            self.end_function();
                        } else {
                            walk::walk_arrow_function_expression(self, arrow);
                        }
                        continue;
                    }
                    walk::walk_variable_declarator(self, declarator);
                }
            }
            Some(oxc_ast::ast::Declaration::FunctionDeclaration(func)) => {
                if let Some(id) = &func.id {
                    let comment = preceding_jsdoc(self.source, decl.span.start);
                    self.begin_function(id.name.as_str(), id.span, func.span, func.r#async, comment.as_deref());
                    walk::walk_function(self, func, ScopeFlags::empty());
                    self.end_function();
                } else {
                    walk::walk_function(self, func, ScopeFlags::empty());
                }
            }
            _ => {
                walk::walk_export_named_declaration(self, decl);
            }
        }
    }

    fn visit_variable_declaration(&mut self, decl: &oxc_ast::ast::VariableDeclaration<'a>) {
        for declarator in &decl.declarations {
            // Record type annotation if present (on the BindingPattern, not BindingIdentifier)
            if let BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind {
                if let Some(type_ann) = &declarator.id.type_annotation {
                    self.record_variable_type(id.name.as_str(), &type_ann.type_annotation);
                }
            }

            if let Some(Expression::ArrowFunctionExpression(arrow)) = &declarator.init {
                if let BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind {
                    let comment = preceding_jsdoc(self.source, decl.span.start);
                    self.begin_function(
                        id.name.as_str(),
                        id.span,
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

        // Visit try block
        walk::walk_block_statement(self, &stmt.block);

        // Visit catch clause with param tracking
        if let Some(handler) = &stmt.handler {
            let catch_param = handler.param.as_ref().and_then(|p| {
                if let BindingPatternKind::BindingIdentifier(id) = &p.pattern.kind {
                    Some(id.name.as_str().to_string())
                } else {
                    None
                }
            });

            if let Some(param) = &catch_param {
                self.catch_param_stack.push(param.clone());
            }

            walk::walk_block_statement(self, &handler.body);

            if catch_param.is_some() {
                self.catch_param_stack.pop();
            }
        }

        // Visit finally block
        if let Some(finalizer) = &stmt.finalizer {
            walk::walk_block_statement(self, finalizer);
        }
    }

    fn visit_ts_interface_declaration(&mut self, iface: &TSInterfaceDeclaration<'a>) {
        self.extract_interface(iface);
        walk::walk_ts_interface_declaration(self, iface);
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        let prev_class = self.current_class.take();
        self.extract_class(class);
        walk::walk_class(self, class);
        self.current_class = prev_class;
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        // Skip abstract methods (they have no body)
        if method.r#type == oxc_ast::ast::MethodDefinitionType::TSAbstractMethodDefinition {
            return;
        }

        let method_name = match &method.key {
            oxc_ast::ast::PropertyKey::StaticIdentifier(id) => id.name.as_str(),
            _ => {
                walk::walk_method_definition(self, method);
                return;
            }
        };

        let name_span = match &method.key {
            oxc_ast::ast::PropertyKey::StaticIdentifier(id) => id.span,
            _ => method.span,
        };

        let comment = preceding_jsdoc(self.source, method.span.start);
        let is_async = method.value.r#async;

        self.begin_function(method_name, name_span, method.value.span, is_async, comment.as_deref());
        walk::walk_method_definition(self, method);
        self.end_function();
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
    let result = extract_all(source, file_path)?;
    Ok(result.signatures)
}

pub fn extract_all(source: &str, file_path: &Path) -> Result<ExtractionResult, ParseError> {
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

    Ok(ExtractionResult {
        signatures: extractor.signatures,
        method_signatures: extractor.method_signatures,
        type_relations: extractor.type_relations,
    })
}

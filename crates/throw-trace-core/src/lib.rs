//! Core engine for throw-trace: types, call graph, propagation analysis.

mod call_graph;
mod diagnostic;
mod propagation;
mod types;

pub use call_graph::CallGraph;
pub use diagnostic::{generate_diagnostics_with_resolver, generate_lsp_violations};
pub use propagation::compute_propagated_throws;
pub use types::{
    CallSite, DeclaredThrow, Diagnostic, ErrorType, FunctionId, FunctionSignature, LspViolation,
    MethodSignature, NoOpTypeResolver, PropagatedThrow, RelationKind, Span, ThrowSite, TryCatchBlock,
    TypeId, TypeRelation, TypeResolver,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn function_id_display() {
        let id = FunctionId {
            file_path: PathBuf::from("src/service.ts"),
            name: "createUser".into(),
            span: Span { start: 10, end: 50 },
        };
        assert_eq!(format!("{id}"), "src/service.ts:createUser");
    }

    #[test]
    fn function_id_anonymous() {
        let id =
            FunctionId::anonymous(PathBuf::from("src/util.ts"), 42, Span { start: 100, end: 150 });
        assert_eq!(id.name.as_str(), "anonymous_L42");
    }

    #[test]
    fn error_type_named() {
        let err = ErrorType::Named("ValidationError".into());
        assert_eq!(err.type_name(), Some("ValidationError"));
    }

    #[test]
    fn error_type_unknown() {
        let err = ErrorType::Unknown;
        assert_eq!(err.type_name(), None);
    }

    #[test]
    fn throw_site_creation() {
        let site = ThrowSite {
            location: Span { start: 100, end: 120 },
            error_type: ErrorType::Named("MyError".into()),
        };
        assert_eq!(site.error_type.type_name(), Some("MyError"));
    }

    #[test]
    fn declared_throw_with_description() {
        let decl = DeclaredThrow {
            error_type: "ValidationError".into(),
            description: Some("When input is invalid".into()),
            span: Span { start: 5, end: 50 },
        };
        assert_eq!(decl.error_type.as_str(), "ValidationError");
        assert!(decl.description.is_some());
    }

    #[test]
    fn call_site_creation() {
        let call = CallSite {
            callee_name: "validate".into(),
            callee_span: Span { start: 200, end: 208 },
            location: Span { start: 200, end: 220 },
        };
        assert_eq!(call.callee_name.as_str(), "validate");
    }

    #[test]
    fn try_catch_block_contains_span() {
        let block = TryCatchBlock {
            try_span: Span { start: 100, end: 200 },
            catch_span: Some(Span { start: 200, end: 300 }),
            caught_types: vec!["ValidationError".into()],
        };
        assert!(block.contains(150));
        assert!(!block.contains(50));
    }

    #[test]
    fn function_signature_creation() {
        let sig = FunctionSignature {
            id: FunctionId::new(
                PathBuf::from("src/test.ts"),
                "testFn",
                Span { start: 0, end: 100 },
            ),
            name_span: Span { start: 9, end: 15 },
            declared_throws: vec![],
            direct_throws: vec![],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        };
        assert_eq!(sig.id.name.as_str(), "testFn");
        assert!(!sig.is_async);
    }

    #[test]
    fn propagated_throw_path() {
        let origin = ThrowSite {
            location: Span { start: 10, end: 30 },
            error_type: ErrorType::Named("DBError".into()),
        };
        let propagated = PropagatedThrow {
            error_type: ErrorType::Named("DBError".into()),
            origin: origin.clone(),
            path: vec![
                FunctionId::new(PathBuf::from("a.ts"), "inner", Span { start: 0, end: 50 }),
                FunctionId::new(PathBuf::from("b.ts"), "outer", Span { start: 0, end: 100 }),
            ],
        };
        assert_eq!(propagated.path.len(), 2);
    }

    #[test]
    fn diagnostic_missing_throws() {
        let func_id = FunctionId::new(
            PathBuf::from("src/service.ts"),
            "createUser",
            Span { start: 0, end: 200 },
        );
        let diagnostic = Diagnostic {
            function: func_id,
            missing_throws: vec![PropagatedThrow {
                error_type: ErrorType::Named("ValidationError".into()),
                origin: ThrowSite {
                    location: Span { start: 50, end: 80 },
                    error_type: ErrorType::Named("ValidationError".into()),
                },
                path: vec![],
            }],
        };
        assert_eq!(diagnostic.missing_throws.len(), 1);
    }

    #[test]
    fn call_graph_add_function() {
        let mut graph = CallGraph::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });
        graph.add_function(id.clone());
        assert!(graph.contains(&id));
    }

    #[test]
    fn call_graph_add_call() {
        let mut graph = CallGraph::new();
        let caller = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });
        let callee = FunctionId::new(PathBuf::from("b.ts"), "bar", Span { start: 0, end: 50 });
        graph.add_function(caller.clone());
        graph.add_function(callee.clone());
        graph.add_call(&caller, &callee);
        let callees = graph.get_callees(&caller);
        assert_eq!(callees.len(), 1);
    }

    #[test]
    fn call_graph_transitive_callees() {
        let mut graph = CallGraph::new();
        let a = FunctionId::new(PathBuf::from("a.ts"), "a", Span { start: 0, end: 50 });
        let b = FunctionId::new(PathBuf::from("b.ts"), "b", Span { start: 0, end: 50 });
        let c = FunctionId::new(PathBuf::from("c.ts"), "c", Span { start: 0, end: 50 });
        graph.add_function(a.clone());
        graph.add_function(b.clone());
        graph.add_function(c.clone());
        graph.add_call(&a, &b);
        graph.add_call(&b, &c);
        let all_callees = graph.get_transitive_callees(&a);
        assert_eq!(all_callees.len(), 2);
    }

    #[test]
    fn propagation_direct_throw() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });
        let sig = FunctionSignature {
            id: id.clone(),
            name_span: Span { start: 9, end: 12 },
            declared_throws: vec![],
            direct_throws: vec![ThrowSite {
                location: Span { start: 10, end: 30 },
                error_type: ErrorType::Named("MyError".into()),
            }],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        };
        signatures.insert(id.clone(), sig);

        let graph = CallGraph::new();
        let propagated = compute_propagated_throws(&id, &signatures, &graph);
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].error_type, ErrorType::Named("MyError".into()));
    }

    #[test]
    fn diagnostic_missing_declaration() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });

        signatures.insert(
            id.clone(),
            FunctionSignature {
                id: id.clone(),
                name_span: Span { start: 9, end: 12 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 10, end: 30 },
                    error_type: ErrorType::Named("MyError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
            },
        );

        let graph = CallGraph::new();
        let diagnostics =
            generate_diagnostics_with_resolver(&signatures, &graph, &mut NoOpTypeResolver);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].missing_throws.len(), 1);
    }

    #[test]
    fn diagnostic_declared_ok() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });

        signatures.insert(
            id.clone(),
            FunctionSignature {
                id: id.clone(),
                name_span: Span { start: 9, end: 12 },
                declared_throws: vec![DeclaredThrow {
                    error_type: "MyError".into(),
                    description: None,
                    span: Span { start: 0, end: 10 },
                }],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 10, end: 30 },
                    error_type: ErrorType::Named("MyError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
            },
        );

        let graph = CallGraph::new();
        let diagnostics =
            generate_diagnostics_with_resolver(&signatures, &graph, &mut NoOpTypeResolver);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn propagation_from_callee() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let mut graph = CallGraph::new();

        let inner = FunctionId::new(PathBuf::from("a.ts"), "inner", Span { start: 0, end: 50 });
        let outer = FunctionId::new(PathBuf::from("b.ts"), "outer", Span { start: 0, end: 100 });

        signatures.insert(
            inner.clone(),
            FunctionSignature {
                id: inner.clone(),
                name_span: Span { start: 9, end: 14 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 10, end: 30 },
                    error_type: ErrorType::Named("InnerError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
            },
        );

        signatures.insert(
            outer.clone(),
            FunctionSignature {
                id: outer.clone(),
                name_span: Span { start: 9, end: 14 },
                declared_throws: vec![],
                direct_throws: vec![],
                calls: vec![CallSite {
                    callee_name: "inner".into(),
                    callee_span: Span { start: 50, end: 55 },
                    location: Span { start: 50, end: 60 },
                }],
                try_catch_blocks: vec![],
                is_async: false,
            },
        );

        graph.add_function(inner.clone());
        graph.add_function(outer.clone());
        graph.add_call(&outer, &inner);

        let propagated = compute_propagated_throws(&outer, &signatures, &graph);
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].error_type, ErrorType::Named("InnerError".into()));
    }

    #[test]
    fn rethrow_in_catch_propagates_original() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        let func = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 200 });

        signatures.insert(
            func.clone(),
            FunctionSignature {
                id: func.clone(),
                name_span: Span { start: 9, end: 12 },
                declared_throws: vec![],
                direct_throws: vec![
                    ThrowSite {
                        location: Span { start: 20, end: 50 },
                        error_type: ErrorType::Named("SomeError".into()),
                    },
                    ThrowSite {
                        location: Span { start: 120, end: 130 },
                        error_type: ErrorType::Rethrow("e".into()),
                    },
                ],
                calls: vec![],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 10, end: 100 },
                    catch_span: Some(Span { start: 100, end: 150 }),
                    caught_types: vec!["SomeError".into()],
                }],
                is_async: false,
            },
        );

        let propagated = compute_propagated_throws(&func, &signatures, &graph);
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].error_type, ErrorType::Named("SomeError".into()));
    }

    #[test]
    fn new_throw_in_catch_propagates() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        let func = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 200 });

        signatures.insert(
            func.clone(),
            FunctionSignature {
                id: func.clone(),
                name_span: Span { start: 9, end: 12 },
                declared_throws: vec![],
                direct_throws: vec![
                    ThrowSite {
                        location: Span { start: 20, end: 50 },
                        error_type: ErrorType::Named("OriginalError".into()),
                    },
                    ThrowSite {
                        location: Span { start: 120, end: 150 },
                        error_type: ErrorType::Named("WrappedError".into()),
                    },
                ],
                calls: vec![],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 10, end: 100 },
                    catch_span: Some(Span { start: 100, end: 180 }),
                    caught_types: vec!["OriginalError".into()],
                }],
                is_async: false,
            },
        );

        let propagated = compute_propagated_throws(&func, &signatures, &graph);
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].error_type, ErrorType::Named("WrappedError".into()));
    }

    #[test]
    fn lsp_violation_detects_undeclared_throw() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        // Class method that throws DBError
        let method_id = FunctionId::new(
            PathBuf::from("test.ts"),
            "findById",
            Span { start: 100, end: 200 },
        );

        signatures.insert(
            method_id.clone(),
            FunctionSignature {
                id: method_id.clone(),
                name_span: Span { start: 110, end: 118 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 150, end: 180 },
                    error_type: ErrorType::Named("DBError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
            },
        );

        // Interface method that declares NotFoundError
        let interface_method = MethodSignature {
            type_id: TypeId::new(
                PathBuf::from("test.ts"),
                "UserRepository",
                Span { start: 0, end: 50 },
            ),
            method_name: "findById".into(),
            method_span: Span { start: 10, end: 40 },
            declared_throws: vec![DeclaredThrow {
                error_type: "NotFoundError".into(),
                description: None,
                span: Span { start: 0, end: 0 },
            }],
            is_abstract: false,
        };

        // Type relation: DatabaseUserRepository implements UserRepository
        let relation = TypeRelation {
            child: TypeId::new(
                PathBuf::from("test.ts"),
                "DatabaseUserRepository",
                Span { start: 60, end: 250 },
            ),
            parent: TypeId::new(
                PathBuf::from("test.ts"),
                "UserRepository",
                Span { start: 0, end: 50 },
            ),
            kind: RelationKind::Implements,
        };

        let violations = generate_lsp_violations(
            &signatures,
            &[interface_method],
            &[relation],
            &graph,
            &mut NoOpTypeResolver,
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].implementation.name.as_str(), "findById");
        assert_eq!(violations[0].illegal_throws.len(), 1);
        assert_eq!(violations[0].illegal_throws[0], ErrorType::Named("DBError".into()));
    }

    #[test]
    fn lsp_violation_allows_declared_throw() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        // Class method that throws NotFoundError (allowed)
        let method_id = FunctionId::new(
            PathBuf::from("test.ts"),
            "findById",
            Span { start: 100, end: 200 },
        );

        signatures.insert(
            method_id.clone(),
            FunctionSignature {
                id: method_id.clone(),
                name_span: Span { start: 110, end: 118 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 150, end: 180 },
                    error_type: ErrorType::Named("NotFoundError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
            },
        );

        // Interface method that declares NotFoundError
        let interface_method = MethodSignature {
            type_id: TypeId::new(
                PathBuf::from("test.ts"),
                "UserRepository",
                Span { start: 0, end: 50 },
            ),
            method_name: "findById".into(),
            method_span: Span { start: 10, end: 40 },
            declared_throws: vec![DeclaredThrow {
                error_type: "NotFoundError".into(),
                description: None,
                span: Span { start: 0, end: 0 },
            }],
            is_abstract: false,
        };

        // Type relation
        let relation = TypeRelation {
            child: TypeId::new(
                PathBuf::from("test.ts"),
                "ImplementationA",
                Span { start: 60, end: 250 },
            ),
            parent: TypeId::new(
                PathBuf::from("test.ts"),
                "UserRepository",
                Span { start: 0, end: 50 },
            ),
            kind: RelationKind::Implements,
        };

        let violations = generate_lsp_violations(
            &signatures,
            &[interface_method],
            &[relation],
            &graph,
            &mut NoOpTypeResolver,
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn lsp_violation_no_throws_declared_means_no_throws_allowed() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        // Class method that throws (not allowed when interface declares nothing)
        let method_id = FunctionId::new(
            PathBuf::from("test.ts"),
            "save",
            Span { start: 100, end: 200 },
        );

        signatures.insert(
            method_id.clone(),
            FunctionSignature {
                id: method_id.clone(),
                name_span: Span { start: 110, end: 114 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 150, end: 180 },
                    error_type: ErrorType::Named("IOError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
            },
        );

        // Interface method with no @throws declaration
        let interface_method = MethodSignature {
            type_id: TypeId::new(
                PathBuf::from("test.ts"),
                "UserRepository",
                Span { start: 0, end: 50 },
            ),
            method_name: "save".into(),
            method_span: Span { start: 10, end: 40 },
            declared_throws: vec![],
            is_abstract: false,
        };

        let relation = TypeRelation {
            child: TypeId::new(
                PathBuf::from("test.ts"),
                "DatabaseUserRepository",
                Span { start: 60, end: 250 },
            ),
            parent: TypeId::new(
                PathBuf::from("test.ts"),
                "UserRepository",
                Span { start: 0, end: 50 },
            ),
            kind: RelationKind::Implements,
        };

        let violations = generate_lsp_violations(
            &signatures,
            &[interface_method],
            &[relation],
            &graph,
            &mut NoOpTypeResolver,
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].illegal_throws[0], ErrorType::Named("IOError".into()));
    }
}

//! Core engine for throw-trace: types, call graph, propagation analysis.

mod call_graph;
mod diagnostic;
mod propagation;
mod types;

pub use call_graph::CallGraph;
pub use diagnostic::{
    find_missing_declarations, generate_diagnostics_with_resolver, generate_lsp_violations,
};
pub use propagation::compute_propagated_throws;
pub use types::{
    CallSite, DeclaredThrow, Diagnostic, ErrorType, FunctionId, FunctionSignature, LspViolation,
    MethodSignature, NoOpTypeResolver, PropagatedThrow, RelationKind, Span, ThrowSite,
    TryCatchBlock, TypeId, TypeRelation, TypeResolver,
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
            class_name: None,
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
        let origin_function =
            FunctionId::new(PathBuf::from("db.ts"), "query", Span { start: 0, end: 40 });
        let propagated = PropagatedThrow {
            error_type: ErrorType::Named("DBError".into()),
            origin: origin.clone(),
            origin_function,
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
            function: func_id.clone(),
            missing_throws: vec![PropagatedThrow {
                error_type: ErrorType::Named("ValidationError".into()),
                origin: ThrowSite {
                    location: Span { start: 50, end: 80 },
                    error_type: ErrorType::Named("ValidationError".into()),
                },
                origin_function: func_id,
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
    #[allow(clippy::similar_names)]
    fn call_graph_add_call() {
        let mut graph = CallGraph::new();
        let span = Span { start: 0, end: 50 };
        let caller = FunctionId::new(PathBuf::from("a.ts"), "foo", span);
        let callee = FunctionId::new(PathBuf::from("b.ts"), "bar", span);
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
            class_name: None,
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
                class_name: None,
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
                class_name: None,
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
                class_name: None,
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
                class_name: None,
            },
        );

        graph.add_function(inner.clone());
        graph.add_function(outer.clone());
        graph.add_call_with_location(&outer, &inner, Span { start: 50, end: 60 });

        let propagated = compute_propagated_throws(&outer, &signatures, &graph);
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].error_type, ErrorType::Named("InnerError".into()));
    }

    // instanceof でマッチした型は rethrow があっても捕捉済みとする
    // rethrow 自体は catch-param の再送出として別途扱われ、
    // マッチしなかった型のみが伝播する
    #[test]
    fn instanceof_matched_type_caught_even_with_rethrow() {
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
                class_name: None,
            },
        );

        let propagated = compute_propagated_throws(&func, &signatures, &graph);
        assert!(
            propagated.is_empty(),
            "instanceof-matched type should be caught even when rethrow exists"
        );
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
                class_name: None,
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
        let method_id =
            FunctionId::new(PathBuf::from("test.ts"), "findById", Span { start: 100, end: 200 });

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
                class_name: Some("DatabaseUserRepository".into()),
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
        let method_id =
            FunctionId::new(PathBuf::from("test.ts"), "findById", Span { start: 100, end: 200 });

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
                class_name: Some("ImplementationA".into()),
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
        let method_id =
            FunctionId::new(PathBuf::from("test.ts"), "save", Span { start: 100, end: 200 });

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
                class_name: Some("DatabaseUserRepository".into()),
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

    // クロスファイル伝播した Unknown throw の型解決は、診断対象関数のファイル
    // ではなく throw 元のファイルに対して行う必要がある。span は throw 元の
    // ソース上のオフセットなので、別ファイルに適用すると解決に失敗する
    #[test]
    fn cross_file_unknown_resolves_against_origin_file() {
        struct OriginFileResolver;
        impl TypeResolver for OriginFileResolver {
            fn is_assignable_to(
                &mut self,
                _file_path: &std::path::Path,
                thrown_type: &str,
                declared_type: &str,
            ) -> bool {
                thrown_type == declared_type
            }
            // 実際の tsserver と同様、throw 元のファイルでのみ型解決が成功する
            fn resolve_type(&mut self, file_path: &std::path::Path, _span: Span) -> Option<String> {
                (file_path == std::path::Path::new("factory.ts")).then(|| "Error".to_string())
            }
        }

        let origin_fn = FunctionId::new(
            PathBuf::from("factory.ts"),
            "createHandler",
            Span { start: 0, end: 300 },
        );
        let caller_fn =
            FunctionId::new(PathBuf::from("setup.ts"), "setup", Span { start: 0, end: 100 });

        let sig = FunctionSignature {
            id: caller_fn.clone(),
            name_span: Span { start: 13, end: 18 },
            declared_throws: vec![DeclaredThrow {
                error_type: "Error".into(),
                description: None,
                span: Span { start: 0, end: 0 },
            }],
            direct_throws: vec![],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
            class_name: None,
        };

        let propagated = vec![PropagatedThrow {
            error_type: ErrorType::Unknown,
            origin: ThrowSite {
                location: Span { start: 217, end: 227 },
                error_type: ErrorType::Unknown,
            },
            origin_function: origin_fn,
            path: vec![caller_fn],
        }];

        let missing = find_missing_declarations(&sig, &propagated, &mut OriginFileResolver);
        assert!(
            missing.is_empty(),
            "declared @throws {{Error}} should satisfy Unknown resolved as Error at origin file, got: {:?}",
            missing.iter().map(|m| &m.error_type).collect::<Vec<_>>()
        );
    }

    // =============================================================
    // catch-all / catch_has_rethrow の伝播判定
    // =============================================================

    #[test]
    fn bare_catch_suppresses_all_throws() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        let func = FunctionId::new(PathBuf::from("a.ts"), "safe", Span { start: 0, end: 200 });

        signatures.insert(
            func.clone(),
            FunctionSignature {
                id: func.clone(),
                name_span: Span { start: 9, end: 13 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 20, end: 50 },
                    error_type: ErrorType::Named("MyError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 10, end: 100 },
                    catch_span: Some(Span { start: 100, end: 180 }),
                    caught_types: vec![],
                }],
                is_async: false,
                class_name: None,
            },
        );

        let propagated = compute_propagated_throws(&func, &signatures, &graph);
        assert!(
            propagated.is_empty(),
            "catch-all should suppress all throws, but got: {:?}",
            propagated.iter().map(|p| &p.error_type).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rethrow_in_bare_catch_propagates_throws() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        let func = FunctionId::new(PathBuf::from("a.ts"), "wrapper", Span { start: 0, end: 200 });

        signatures.insert(
            func.clone(),
            FunctionSignature {
                id: func.clone(),
                name_span: Span { start: 9, end: 16 },
                declared_throws: vec![],
                direct_throws: vec![
                    ThrowSite {
                        location: Span { start: 20, end: 50 },
                        error_type: ErrorType::Named("MyError".into()),
                    },
                    ThrowSite {
                        location: Span { start: 120, end: 130 },
                        error_type: ErrorType::Rethrow("e".into()),
                    },
                ],
                calls: vec![],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 10, end: 100 },
                    catch_span: Some(Span { start: 100, end: 180 }),
                    caught_types: vec![],
                }],
                is_async: false,
                class_name: None,
            },
        );

        let propagated = compute_propagated_throws(&func, &signatures, &graph);
        assert_eq!(propagated.len(), 1, "catch-all with rethrow should still propagate");
        assert_eq!(propagated[0].error_type, ErrorType::Named("MyError".into()));
    }

    #[test]
    fn rethrow_in_catch_does_not_discard_instanceof_match() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let graph = CallGraph::new();

        let func =
            FunctionId::new(PathBuf::from("a.ts"), "handleTarget", Span { start: 0, end: 300 });

        signatures.insert(
            func.clone(),
            FunctionSignature {
                id: func.clone(),
                name_span: Span { start: 9, end: 21 },
                declared_throws: vec![],
                direct_throws: vec![
                    ThrowSite {
                        location: Span { start: 20, end: 50 },
                        error_type: ErrorType::Named("TargetError".into()),
                    },
                    ThrowSite {
                        location: Span { start: 220, end: 230 },
                        error_type: ErrorType::Rethrow("e".into()),
                    },
                ],
                calls: vec![],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 10, end: 100 },
                    catch_span: Some(Span { start: 100, end: 250 }),
                    caught_types: vec!["TargetError".into()],
                }],
                is_async: false,
                class_name: None,
            },
        );

        let propagated = compute_propagated_throws(&func, &signatures, &graph);
        assert!(
            propagated.is_empty(),
            "TargetError is caught by instanceof, should not propagate, but got: {:?}",
            propagated.iter().map(|p| &p.error_type).collect::<Vec<_>>()
        );
    }

    // 同名関数を2回呼び、1回目だけ try-catch 内にあるケース
    // 2回目は裸呼び出しなので throw が伝播するはず
    #[test]
    #[allow(clippy::similar_names)]
    fn duplicate_call_uncaught_outside_try() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let mut graph = CallGraph::new();

        let callee = FunctionId::new(PathBuf::from("a.ts"), "risky", Span { start: 0, end: 50 });
        let caller =
            FunctionId::new(PathBuf::from("a.ts"), "callTwice", Span { start: 100, end: 400 });

        signatures.insert(
            callee.clone(),
            FunctionSignature {
                id: callee.clone(),
                name_span: Span { start: 9, end: 14 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 20, end: 40 },
                    error_type: ErrorType::Named("SomeError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
                class_name: None,
            },
        );

        signatures.insert(
            caller.clone(),
            FunctionSignature {
                id: caller.clone(),
                name_span: Span { start: 109, end: 118 },
                declared_throws: vec![],
                direct_throws: vec![],
                calls: vec![
                    CallSite {
                        callee_name: "risky".into(),
                        callee_span: Span { start: 150, end: 155 },
                        location: Span { start: 150, end: 160 },
                    },
                    CallSite {
                        callee_name: "risky".into(),
                        callee_span: Span { start: 300, end: 305 },
                        location: Span { start: 300, end: 310 },
                    },
                ],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 130, end: 200 },
                    catch_span: Some(Span { start: 200, end: 250 }),
                    caught_types: vec![],
                }],
                is_async: false,
                class_name: None,
            },
        );

        graph.add_function(callee.clone());
        graph.add_function(caller.clone());
        graph.add_call_with_location(&caller, &callee, Span { start: 150, end: 160 });
        graph.add_call_with_location(&caller, &callee, Span { start: 300, end: 310 });

        let propagated = compute_propagated_throws(&caller, &signatures, &graph);
        assert_eq!(
            propagated.len(),
            1,
            "second call to risky() is outside try-catch, should propagate"
        );
    }

    // 2つの呼び出しパスが同じ推移的 callee を共有するケース。
    // caller → a → f (a() は try-catch 内 → 捕捉される)
    // caller → b → f (b() は try-catch 外 → 伝播するはず)
    // visited の書き戻し方式だと、先に処理されたパスで f が訪問済みになり、
    // 後続パスの throw が収集されず false negative になる
    #[test]
    #[allow(clippy::similar_names, clippy::too_many_lines)]
    fn shared_transitive_callee_propagates_via_uncaught_path() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let mut graph = CallGraph::new();

        let f = FunctionId::new(PathBuf::from("f.ts"), "f", Span { start: 0, end: 50 });
        let a = FunctionId::new(PathBuf::from("a.ts"), "a", Span { start: 0, end: 100 });
        let b = FunctionId::new(PathBuf::from("b.ts"), "b", Span { start: 0, end: 100 });
        let caller =
            FunctionId::new(PathBuf::from("main.ts"), "caller", Span { start: 0, end: 400 });

        signatures.insert(
            f.clone(),
            FunctionSignature {
                id: f.clone(),
                name_span: Span { start: 9, end: 10 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 20, end: 40 },
                    error_type: ErrorType::Named("SharedError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
                class_name: None,
            },
        );

        for (id, call_loc) in [(&a, 50), (&b, 50)] {
            signatures.insert(
                (*id).clone(),
                FunctionSignature {
                    id: (*id).clone(),
                    name_span: Span { start: 9, end: 10 },
                    declared_throws: vec![],
                    direct_throws: vec![],
                    calls: vec![CallSite {
                        callee_name: "f".into(),
                        callee_span: Span { start: call_loc, end: call_loc + 1 },
                        location: Span { start: call_loc, end: call_loc + 10 },
                    }],
                    try_catch_blocks: vec![],
                    is_async: false,
                    class_name: None,
                },
            );
        }

        // caller: a() は try-catch 内 (130-200)、b() は外 (300)
        signatures.insert(
            caller.clone(),
            FunctionSignature {
                id: caller.clone(),
                name_span: Span { start: 9, end: 15 },
                declared_throws: vec![],
                direct_throws: vec![],
                calls: vec![
                    CallSite {
                        callee_name: "a".into(),
                        callee_span: Span { start: 150, end: 151 },
                        location: Span { start: 150, end: 160 },
                    },
                    CallSite {
                        callee_name: "b".into(),
                        callee_span: Span { start: 300, end: 301 },
                        location: Span { start: 300, end: 310 },
                    },
                ],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 130, end: 200 },
                    catch_span: Some(Span { start: 200, end: 250 }),
                    caught_types: vec![],
                }],
                is_async: false,
                class_name: None,
            },
        );

        graph.add_function(f.clone());
        graph.add_function(a.clone());
        graph.add_function(b.clone());
        graph.add_function(caller.clone());
        graph.add_call_with_location(&a, &f, Span { start: 50, end: 60 });
        graph.add_call_with_location(&b, &f, Span { start: 50, end: 60 });
        // petgraph は後に追加したエッジから列挙するため、b → a の順で追加して
        // 捕捉される側のパス (a) が先に処理されるようにする
        graph.add_call_with_location(&caller, &b, Span { start: 300, end: 310 });
        graph.add_call_with_location(&caller, &a, Span { start: 150, end: 160 });

        let propagated = compute_propagated_throws(&caller, &signatures, &graph);
        assert_eq!(
            propagated.len(),
            1,
            "SharedError should propagate via b() (outside try-catch). Got: {:?}",
            propagated.iter().map(|p| &p.error_type).collect::<Vec<_>>()
        );
        assert_eq!(propagated[0].error_type, ErrorType::Named("SharedError".into()));
        assert_eq!(propagated[0].path, vec![caller.clone(), b.clone()]);
    }

    // 同名メソッド（a.find() と b.find()）が別の関数を指すケース。
    // a.find() は throw する側で try-catch 内、b.find() は throw しない側で try-catch 外。
    // 名前ベースマッチングだと b.find() の call site span も a_find のエッジで拾われ、
    // try-catch 外にある → uncaught と誤判定される。
    #[test]
    #[allow(clippy::similar_names)]
    fn same_name_different_callee_false_positive() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let mut graph = CallGraph::new();

        // a.find() — throws
        let a_find = FunctionId::new(PathBuf::from("a.ts"), "find", Span { start: 0, end: 50 });
        // b.find() — does NOT throw
        let b_find = FunctionId::new(PathBuf::from("b.ts"), "find", Span { start: 0, end: 50 });

        let caller =
            FunctionId::new(PathBuf::from("main.ts"), "caller", Span { start: 0, end: 400 });

        signatures.insert(
            a_find.clone(),
            FunctionSignature {
                id: a_find.clone(),
                name_span: Span { start: 9, end: 13 },
                declared_throws: vec![],
                direct_throws: vec![ThrowSite {
                    location: Span { start: 20, end: 40 },
                    error_type: ErrorType::Named("NotFoundError".into()),
                }],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
                class_name: None,
            },
        );

        signatures.insert(
            b_find.clone(),
            FunctionSignature {
                id: b_find.clone(),
                name_span: Span { start: 9, end: 13 },
                declared_throws: vec![],
                direct_throws: vec![],
                calls: vec![],
                try_catch_blocks: vec![],
                is_async: false,
                class_name: None,
            },
        );

        // caller: a.find() in try-catch (span 100-200), b.find() outside (span 300)
        signatures.insert(
            caller.clone(),
            FunctionSignature {
                id: caller.clone(),
                name_span: Span { start: 9, end: 15 },
                declared_throws: vec![],
                direct_throws: vec![],
                calls: vec![
                    CallSite {
                        callee_name: "find".into(),
                        callee_span: Span { start: 150, end: 154 },
                        location: Span { start: 150, end: 160 },
                    },
                    CallSite {
                        callee_name: "find".into(),
                        callee_span: Span { start: 300, end: 304 },
                        location: Span { start: 300, end: 310 },
                    },
                ],
                try_catch_blocks: vec![TryCatchBlock {
                    try_span: Span { start: 100, end: 200 },
                    catch_span: Some(Span { start: 200, end: 250 }),
                    caught_types: vec![],
                }],
                is_async: false,
                class_name: None,
            },
        );

        graph.add_function(a_find.clone());
        graph.add_function(b_find.clone());
        graph.add_function(caller.clone());
        // call graph edges with call site locations
        graph.add_call_with_location(&caller, &a_find, Span { start: 150, end: 160 });
        graph.add_call_with_location(&caller, &b_find, Span { start: 300, end: 310 });

        let propagated = compute_propagated_throws(&caller, &signatures, &graph);
        // a.find() is in try-catch → caught → should NOT propagate
        // b.find() does NOT throw → nothing to propagate
        // Expected: 0 propagated throws
        // Actual with name-based matching: 1 (false positive)
        // because b.find()'s call site (outside try) is matched to a_find's edge
        assert_eq!(
            propagated.len(),
            0,
            "a.find() is caught in try-catch, b.find() does not throw, \
             but name-based matching causes false positive. Got: {:?}",
            propagated.iter().map(|p| &p.error_type).collect::<Vec<_>>()
        );
    }
}

use crate::{
    compute_propagated_throws, CallGraph, Diagnostic, ErrorType, FunctionId, FunctionSignature,
    LspViolation, MethodSignature, PropagatedThrow, TypeParam, TypeRelation, TypeResolver,
};
use std::collections::HashMap;

pub fn generate_diagnostics_with_resolver<S: std::hash::BuildHasher, R: TypeResolver>(
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    graph: &CallGraph,
    resolver: &mut R,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (func_id, sig) in signatures {
        let propagated = compute_propagated_throws(func_id, signatures, graph);
        let missing = find_missing_declarations(sig, &propagated, signatures, resolver);

        if !missing.is_empty() {
            diagnostics.push(Diagnostic { function: func_id.clone(), missing_throws: missing });
        }
    }

    diagnostics
}

pub fn find_missing_declarations<R: TypeResolver, S: std::hash::BuildHasher>(
    sig: &FunctionSignature,
    propagated: &[PropagatedThrow],
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    resolver: &mut R,
) -> Vec<PropagatedThrow> {
    let declared_types: Vec<&str> =
        sig.declared_throws.iter().map(|d| d.error_type.as_str()).collect();

    propagated
        .iter()
        .filter_map(|p| {
            // 型パラメータの constraint 置換は throw が実際に書かれた関数
            // (origin_function) のスコープに基づく必要がある。伝播元 (sig) の
            // 型パラメータではなく、origin_function の型パラメータを参照する。
            let origin_type_params: &[TypeParam] = signatures
                .get(&p.origin_function)
                .map_or(&[], |origin_sig| origin_sig.type_params.as_slice());

            let (is_decl, resolved_type) = is_declared_with_resolution(
                &p.error_type,
                p.origin.location,
                &p.origin_function.file_path,
                &declared_types,
                origin_type_params,
                resolver,
            );
            if is_decl {
                None
            } else {
                let mut result = p.clone();
                if let Some(resolved) = resolved_type {
                    result.error_type = ErrorType::Named(resolved.into());
                }
                Some(result)
            }
        })
        .collect()
}

fn is_declared_with_resolution<R: TypeResolver>(
    error_type: &ErrorType,
    throw_span: crate::Span,
    file_path: &std::path::Path,
    declared_types: &[&str],
    origin_type_params: &[TypeParam],
    type_resolver: &mut R,
) -> (bool, Option<String>) {
    match error_type {
        ErrorType::Named(thrown_type) => {
            let is_decl = declared_types
                .iter()
                .any(|declared| type_resolver.is_assignable_to(file_path, thrown_type, declared));
            (is_decl, None)
        }
        ErrorType::Unknown => {
            let Some(resolved_type) = type_resolver.resolve_type(file_path, throw_span) else {
                let is_decl = declared_types.iter().any(|declared| *declared == "unknown");
                return (is_decl, None);
            };

            match type_param_constraint(&resolved_type, origin_type_params) {
                // 制約付き型パラメータ (`E extends C`): C への throw として扱う
                TypeParamMatch::Constrained(constraint) => {
                    let is_decl = declared_types.iter().any(|declared| {
                        type_resolver.is_assignable_to(file_path, constraint, declared)
                    });
                    (is_decl, Some(constraint.to_string()))
                }
                // 制約なし型パラメータ (`E`): 暗黙の制約 unknown として扱う
                TypeParamMatch::Unconstrained => {
                    let is_decl = declared_types.iter().any(|declared| *declared == "unknown");
                    (is_decl, None)
                }
                // 型パラメータ由来ではない通常の型解決結果
                TypeParamMatch::NotATypeParam => {
                    let is_decl = declared_types.iter().any(|declared| {
                        type_resolver.is_assignable_to(file_path, &resolved_type, declared)
                    });
                    (is_decl, Some(resolved_type))
                }
            }
        }
        ErrorType::Rethrow(_) => (false, None),
    }
}

enum TypeParamMatch<'a> {
    /// 型パラメータ名ではない通常の型解決結果
    NotATypeParam,
    /// 制約なし型パラメータ (`E`)
    Unconstrained,
    /// 制約付き型パラメータ (`E extends C`)。値は constraint (`C`) のテキスト
    Constrained(&'a str),
}

// resolve_type が返す quickinfo 由来の生テキストが、throw 元関数自身が宣言した
// 型パラメータ名 (`E`) または `E extends C` の形と一致するかを判定する。
fn type_param_constraint<'a>(
    resolved_type: &str,
    type_params: &'a [TypeParam],
) -> TypeParamMatch<'a> {
    for tp in type_params {
        let name = tp.name.as_str();
        let matches = resolved_type == name
            || resolved_type
                .strip_prefix(name)
                .is_some_and(|rest| rest.trim_start().starts_with("extends"));
        if !matches {
            continue;
        }
        return match tp.constraint.as_deref() {
            Some(constraint) => TypeParamMatch::Constrained(constraint),
            None => TypeParamMatch::Unconstrained,
        };
    }
    TypeParamMatch::NotATypeParam
}

pub fn generate_lsp_violations<S: std::hash::BuildHasher, R: TypeResolver>(
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    method_signatures: &[MethodSignature],
    type_relations: &[TypeRelation],
    graph: &CallGraph,
    resolver: &mut R,
) -> Vec<LspViolation> {
    let mut violations = Vec::new();

    // Build lookup: type name -> parent types
    let parent_lookup = build_parent_lookup(type_relations);

    // Build lookup: (type name, method name) -> method signature
    let method_lookup = build_method_lookup(method_signatures);

    for (func_id, sig) in signatures {
        // Find the class this function belongs to
        let class_name = extract_class_name_from_signature(func_id, signatures);
        let Some(class_name) = class_name else {
            continue;
        };

        // Get all parent types (direct + transitive)
        let parent_types = get_all_parent_types(&class_name, &parent_lookup);

        // For each parent type, check if there's a method with the same name
        for parent_type in &parent_types {
            let key = (parent_type.as_str(), func_id.name.as_str());
            let Some(parent_method) = method_lookup.get(&key) else {
                continue;
            };

            // Get propagated throws for this function
            let propagated = compute_propagated_throws(func_id, signatures, graph);

            // Check each propagated throw against parent's declared throws
            let illegal =
                find_illegal_throws(&propagated, parent_method, resolver, &sig.id.file_path);

            if !illegal.is_empty() {
                violations.push(LspViolation {
                    implementation: func_id.clone(),
                    parent_method: (*parent_method).clone(),
                    illegal_throws: illegal,
                });
            }
        }
    }

    violations
}

fn build_parent_lookup(relations: &[TypeRelation]) -> HashMap<String, Vec<String>> {
    let mut lookup: HashMap<String, Vec<String>> = HashMap::new();
    for rel in relations {
        lookup.entry(rel.child.name.to_string()).or_default().push(rel.parent.name.to_string());
    }
    lookup
}

fn build_method_lookup(methods: &[MethodSignature]) -> HashMap<(&str, &str), &MethodSignature> {
    methods.iter().map(|m| ((m.type_id.name.as_str(), m.method_name.as_str()), m)).collect()
}

fn extract_class_name_from_signature<S: std::hash::BuildHasher>(
    func_id: &FunctionId,
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
) -> Option<String> {
    signatures.get(func_id).and_then(|sig| sig.class_name.as_ref().map(ToString::to_string))
}

fn get_all_parent_types(type_name: &str, lookup: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue = vec![type_name.to_string()];

    while let Some(current) = queue.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());

        if let Some(parents) = lookup.get(&current) {
            for parent in parents {
                result.push(parent.clone());
                queue.push(parent.clone());
            }
        }
    }

    result
}

fn find_illegal_throws<R: TypeResolver>(
    propagated: &[PropagatedThrow],
    parent_method: &MethodSignature,
    resolver: &mut R,
    file_path: &std::path::Path,
) -> Vec<ErrorType> {
    let declared_types: Vec<&str> =
        parent_method.declared_throws.iter().map(|d| d.error_type.as_str()).collect();

    propagated
        .iter()
        .filter_map(|p| {
            match &p.error_type {
                ErrorType::Named(thrown_type) => {
                    let is_allowed = declared_types.iter().any(|declared| {
                        resolver.is_assignable_to(file_path, thrown_type, declared)
                    });
                    if is_allowed {
                        None
                    } else {
                        Some(p.error_type.clone())
                    }
                }
                ErrorType::Unknown | ErrorType::Rethrow(_) => {
                    // Unknown throws are always violations if parent declares nothing or specific types
                    if declared_types.is_empty() {
                        Some(p.error_type.clone())
                    } else {
                        None
                    }
                }
            }
        })
        .collect()
}

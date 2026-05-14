use crate::{
    compute_propagated_throws, CallGraph, Diagnostic, ErrorType, FunctionId, FunctionSignature,
    LspViolation, MethodSignature, PropagatedThrow, TypeRelation, TypeResolver,
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
        let missing = find_missing_declarations(sig, &propagated, resolver);

        if !missing.is_empty() {
            diagnostics.push(Diagnostic { function: func_id.clone(), missing_throws: missing });
        }
    }

    diagnostics
}

fn find_missing_declarations<R: TypeResolver>(
    sig: &FunctionSignature,
    propagated: &[PropagatedThrow],
    resolver: &mut R,
) -> Vec<PropagatedThrow> {
    let declared_types: Vec<&str> =
        sig.declared_throws.iter().map(|d| d.error_type.as_str()).collect();

    propagated
        .iter()
        .filter_map(|p| {
            let (is_decl, resolved_type) = is_declared_with_resolution(
                &p.error_type,
                &p.origin.location,
                &sig.id.file_path,
                &declared_types,
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
    throw_span: &crate::Span,
    file_path: &std::path::PathBuf,
    declared_types: &[&str],
    resolver: &mut R,
) -> (bool, Option<String>) {
    match error_type {
        ErrorType::Named(thrown_type) => {
            let is_decl = declared_types
                .iter()
                .any(|declared| resolver.is_assignable_to(file_path, thrown_type, declared));
            (is_decl, None)
        }
        ErrorType::Unknown => {
            let Some(resolved) = resolver.resolve_type(file_path, *throw_span) else {
                return (false, None);
            };
            let is_decl = declared_types
                .iter()
                .any(|declared| resolver.is_assignable_to(file_path, &resolved, declared));
            (is_decl, Some(resolved))
        }
        ErrorType::Rethrow(_) => (false, None),
    }
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
        // Find the class this function belongs to (by matching file path and checking if
        // function name matches a method in a class that has type relations)
        let class_name = extract_class_name_from_function(func_id, type_relations);
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
            let illegal = find_illegal_throws(&propagated, parent_method, resolver, &sig.id.file_path);

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
        lookup
            .entry(rel.child.name.to_string())
            .or_default()
            .push(rel.parent.name.to_string());
    }
    lookup
}

fn build_method_lookup(methods: &[MethodSignature]) -> HashMap<(&str, &str), &MethodSignature> {
    methods.iter().map(|m| ((m.type_id.name.as_str(), m.method_name.as_str()), m)).collect()
}

fn extract_class_name_from_function(
    func_id: &FunctionId,
    relations: &[TypeRelation],
) -> Option<String> {
    // Find if there's a type relation where the child class is in the same file
    // and the function could be a method of that class
    for rel in relations {
        if rel.child.file_path == func_id.file_path {
            return Some(rel.child.name.to_string());
        }
    }
    None
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
    file_path: &std::path::PathBuf,
) -> Vec<ErrorType> {
    let declared_types: Vec<&str> =
        parent_method.declared_throws.iter().map(|d| d.error_type.as_str()).collect();

    propagated
        .iter()
        .filter_map(|p| {
            match &p.error_type {
                ErrorType::Named(thrown_type) => {
                    let is_allowed = declared_types
                        .iter()
                        .any(|declared| resolver.is_assignable_to(file_path, thrown_type, declared));
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

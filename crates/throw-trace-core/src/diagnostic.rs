use crate::{
    compute_propagated_throws, CallGraph, Diagnostic, ErrorType, FunctionId, FunctionSignature,
    PropagatedThrow, TypeResolver,
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

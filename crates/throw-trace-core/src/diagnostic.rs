use crate::{
    compute_propagated_throws, CallGraph, Diagnostic, ErrorType, FunctionId, FunctionSignature,
    PropagatedThrow,
};
use std::collections::HashMap;

pub fn generate_diagnostics<S: std::hash::BuildHasher>(
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    graph: &CallGraph,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (func_id, sig) in signatures {
        let propagated = compute_propagated_throws(func_id, signatures, graph);
        let missing = find_missing_declarations(sig, &propagated);

        if !missing.is_empty() {
            diagnostics.push(Diagnostic { function: func_id.clone(), missing_throws: missing });
        }
    }

    diagnostics
}

fn find_missing_declarations(
    sig: &FunctionSignature,
    propagated: &[PropagatedThrow],
) -> Vec<PropagatedThrow> {
    propagated.iter().filter(|p| !is_declared(&p.error_type, sig)).cloned().collect()
}

fn is_declared(error_type: &ErrorType, sig: &FunctionSignature) -> bool {
    let ErrorType::Named(type_name) = error_type else {
        return false;
    };

    sig.declared_throws.iter().any(|d| d.error_type.as_str() == type_name.as_str())
}

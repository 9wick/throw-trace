use crate::{CallGraph, ErrorType, FunctionId, FunctionSignature, PropagatedThrow, ThrowSite};
use std::collections::{HashMap, HashSet};

pub fn compute_propagated_throws<S: std::hash::BuildHasher>(
    func_id: &FunctionId,
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    graph: &CallGraph,
) -> Vec<PropagatedThrow> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();

    collect_throws(func_id, signatures, graph, &mut result, &mut visited, &[]);

    result
}

fn collect_throws<S: std::hash::BuildHasher>(
    func_id: &FunctionId,
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    graph: &CallGraph,
    result: &mut Vec<PropagatedThrow>,
    visited: &mut HashSet<FunctionId>,
    path: &[FunctionId],
) {
    if visited.contains(func_id) {
        return;
    }
    visited.insert(func_id.clone());

    let Some(sig) = signatures.get(func_id) else {
        return;
    };

    for throw_site in &sig.direct_throws {
        if !is_caught(throw_site, sig) {
            result.push(PropagatedThrow {
                error_type: throw_site.error_type.clone(),
                origin: throw_site.clone(),
                path: path.to_owned(),
            });
        }
    }

    for callee_id in graph.get_callees(func_id) {
        let mut new_path = path.to_owned();
        new_path.push(func_id.clone());
        collect_throws(&callee_id, signatures, graph, result, visited, &new_path);
    }
}

fn is_caught(throw_site: &ThrowSite, sig: &FunctionSignature) -> bool {
    for block in &sig.try_catch_blocks {
        if !block.contains(throw_site.location.start) {
            continue;
        }

        if let ErrorType::Named(throw_type) = &throw_site.error_type {
            if block.caught_types.iter().any(|t| t == throw_type) {
                return true;
            }
        }
    }
    false
}

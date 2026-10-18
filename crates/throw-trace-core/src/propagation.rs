use crate::{
    CallGraph, ErrorType, FunctionId, FunctionSignature, PropagatedThrow, Span, ThrowSite,
    TryCatchBlock,
};
use std::collections::{HashMap, HashSet};

pub fn compute_propagated_throws<S: std::hash::BuildHasher>(
    func_id: &FunctionId,
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    graph: &CallGraph,
) -> Vec<PropagatedThrow> {
    let mut memo = HashMap::new();
    let mut in_progress = HashSet::new();
    let mut blocked = HashSet::new();
    escaping_throws(func_id, signatures, graph, &mut memo, &mut in_progress, &mut blocked)
}

// 関数から外へ漏れる throw を計算する。path は当該関数を root とした相対パスで
// 保持し、呼び出し元が取り込む際に自身を先頭へ付け足すことで絶対パスになる。
// 同一の callee を複数パスが共有しても結果が欠落しないよう、訪問済みフラグの
// 共有ではなく関数単位のメモ化で重複計算を防ぐ。
fn escaping_throws<S: std::hash::BuildHasher>(
    func_id: &FunctionId,
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    graph: &CallGraph,
    memo: &mut HashMap<FunctionId, Vec<PropagatedThrow>>,
    in_progress: &mut HashSet<FunctionId>,
    blocked: &mut HashSet<FunctionId>,
) -> Vec<PropagatedThrow> {
    if let Some(cached) = memo.get(func_id) {
        return cached.clone();
    }
    if !in_progress.insert(func_id.clone()) {
        // 循環呼び出し: 計算中ノードへの再突入は打ち切る（1パス近似）
        blocked.insert(func_id.clone());
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut subtree_blocked = HashSet::new();

    if let Some(sig) = signatures.get(func_id) {
        for throw_site in &sig.direct_throws {
            if let ErrorType::CapturedRethrow { function, try_span } = &throw_site.error_type {
                for mut source in caught_sources(
                    function,
                    *try_span,
                    signatures,
                    graph,
                    memo,
                    in_progress,
                    &mut subtree_blocked,
                ) {
                    let rethrow = ThrowSite {
                        location: throw_site.location,
                        error_type: source.error_type.clone(),
                    };
                    if !is_caught(&rethrow, sig) {
                        source.path.insert(0, func_id.clone());
                        result.push(source);
                    }
                }
                continue;
            }
            if throw_site.error_type.is_rethrow() && is_in_catch_block(throw_site, sig) {
                continue;
            }
            if !is_caught(throw_site, sig) {
                result.push(PropagatedThrow {
                    error_type: throw_site.error_type.clone(),
                    origin: throw_site.clone(),
                    origin_function: func_id.clone(),
                    path: Vec::new(),
                });
            }
        }

        for callee_id in graph.get_callees(func_id) {
            let callee_throws = escaping_throws(
                &callee_id,
                signatures,
                graph,
                memo,
                in_progress,
                &mut subtree_blocked,
            );

            let call_site_locations = graph.get_call_site_locations(func_id, &callee_id);

            for propagated in callee_throws {
                let any_uncaught = if call_site_locations.is_empty() {
                    true
                } else {
                    call_site_locations.iter().any(|&span| {
                        let virtual_throw =
                            ThrowSite { location: span, error_type: propagated.error_type.clone() };
                        !is_caught(&virtual_throw, sig)
                    })
                };
                if any_uncaught {
                    let mut path = Vec::with_capacity(propagated.path.len() + 1);
                    path.push(func_id.clone());
                    path.extend(propagated.path);
                    result.push(PropagatedThrow { path, ..propagated });
                }
            }
        }
    }

    in_progress.remove(func_id);
    subtree_blocked.remove(func_id);
    // 循環中に打ち切られたノードに依存した結果は不完全なのでメモ化しない
    if subtree_blocked.is_empty() {
        memo.insert(func_id.clone(), result.clone());
    } else {
        blocked.extend(subtree_blocked);
    }
    result
}

fn caught_sources<S: std::hash::BuildHasher>(
    function: &FunctionId,
    try_span: Span,
    signatures: &HashMap<FunctionId, FunctionSignature, S>,
    graph: &CallGraph,
    memo: &mut HashMap<FunctionId, Vec<PropagatedThrow>>,
    in_progress: &mut HashSet<FunctionId>,
    blocked: &mut HashSet<FunctionId>,
) -> Vec<PropagatedThrow> {
    let mut sources = Vec::new();
    let Some(sig) = signatures.get(function) else { return sources };
    for site in &sig.direct_throws {
        if site.location.start < try_span.start
            || site.location.start >= try_span.end
            || site.error_type.is_rethrow()
        {
            continue;
        }
        if !is_caught_within(site, sig, Some(try_span)) {
            if let ErrorType::CapturedRethrow { function, try_span: captured_span } =
                &site.error_type
            {
                for source in caught_sources(
                    function,
                    *captured_span,
                    signatures,
                    graph,
                    memo,
                    in_progress,
                    blocked,
                ) {
                    let rethrow = ThrowSite {
                        location: site.location,
                        error_type: source.error_type.clone(),
                    };
                    if !is_caught_within(&rethrow, sig, Some(try_span)) {
                        sources.push(source);
                    }
                }
            } else {
                sources.push(PropagatedThrow {
                    error_type: site.error_type.clone(),
                    origin: site.clone(),
                    origin_function: function.clone(),
                    path: Vec::new(),
                });
            }
        }
    }
    for callee in graph.get_callees(function) {
        let locations: Vec<_> = graph
            .get_call_site_locations(function, &callee)
            .iter()
            .filter(|span| span.start >= try_span.start && span.start < try_span.end)
            .collect();
        if locations.is_empty() {
            continue;
        }
        for source in escaping_throws(&callee, signatures, graph, memo, in_progress, blocked) {
            if locations.iter().any(|location| {
                !is_caught_within(
                    &ThrowSite { location: **location, error_type: source.error_type.clone() },
                    sig,
                    Some(try_span),
                )
            }) {
                sources.push(source);
            }
        }
    }
    sources
}

fn is_caught(throw_site: &ThrowSite, sig: &FunctionSignature) -> bool {
    is_caught_within(throw_site, sig, None)
}

fn is_caught_within(throw_site: &ThrowSite, sig: &FunctionSignature, within: Option<Span>) -> bool {
    if throw_site.error_type.is_rethrow() {
        return false;
    }

    for block in &sig.try_catch_blocks {
        if let Some(outer) = within {
            if block.try_span == outer
                || block.try_span.start < outer.start
                || block.try_span.end > outer.end
            {
                continue;
            }
        }
        if !block.contains(throw_site.location.start) {
            continue;
        }

        let has_rethrow = catch_has_rethrow(block, sig);

        if block.caught_types.is_empty() && !has_rethrow {
            return true;
        }

        if let ErrorType::Named(throw_type) = &throw_site.error_type {
            if block.caught_types.iter().any(|t| t == throw_type) {
                return true;
            }
        }
    }
    false
}

fn catch_has_rethrow(block: &TryCatchBlock, sig: &FunctionSignature) -> bool {
    let Some(catch_span) = &block.catch_span else {
        return false;
    };

    sig.direct_throws.iter().any(|throw_site| {
        throw_site.error_type.is_rethrow()
            && throw_site.location.start >= catch_span.start
            && throw_site.location.end <= catch_span.end
    })
}

fn is_in_catch_block(throw_site: &ThrowSite, sig: &FunctionSignature) -> bool {
    sig.try_catch_blocks.iter().any(|block| {
        if let Some(catch_span) = &block.catch_span {
            throw_site.location.start >= catch_span.start
                && throw_site.location.end <= catch_span.end
        } else {
            false
        }
    })
}

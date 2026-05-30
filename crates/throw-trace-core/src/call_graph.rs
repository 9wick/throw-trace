use crate::{FunctionId, Span};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

pub struct CallGraph {
    graph: DiGraph<FunctionId, ()>,
    node_map: HashMap<FunctionId, NodeIndex>,
    call_site_locations: HashMap<(NodeIndex, NodeIndex), Vec<Span>>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
            call_site_locations: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, id: FunctionId) {
        if let std::collections::hash_map::Entry::Vacant(e) = self.node_map.entry(id) {
            let idx = self.graph.add_node(e.key().clone());
            e.insert(idx);
        }
    }

    pub fn contains(&self, id: &FunctionId) -> bool {
        self.node_map.contains_key(id)
    }

    pub fn add_call(&mut self, from: &FunctionId, to: &FunctionId) {
        if let (Some(&from_idx), Some(&to_idx)) = (self.node_map.get(from), self.node_map.get(to)) {
            if !self.graph.contains_edge(from_idx, to_idx) {
                self.graph.add_edge(from_idx, to_idx, ());
            }
        }
    }

    pub fn add_call_with_location(
        &mut self,
        from: &FunctionId,
        to: &FunctionId,
        call_location: Span,
    ) {
        if let (Some(&from_idx), Some(&to_idx)) = (self.node_map.get(from), self.node_map.get(to)) {
            if !self.graph.contains_edge(from_idx, to_idx) {
                self.graph.add_edge(from_idx, to_idx, ());
            }
            self.call_site_locations.entry((from_idx, to_idx)).or_default().push(call_location);
        }
    }

    pub fn get_call_site_locations(&self, from: &FunctionId, to: &FunctionId) -> &[Span] {
        let (Some(&from_idx), Some(&to_idx)) = (self.node_map.get(from), self.node_map.get(to))
        else {
            return &[];
        };
        self.call_site_locations
            .get(&(from_idx, to_idx))
            .map_or(&[], Vec::as_slice)
    }

    pub fn get_callees(&self, caller: &FunctionId) -> Vec<FunctionId> {
        let Some(&caller_idx) = self.node_map.get(caller) else {
            return Vec::new();
        };

        self.graph
            .neighbors(caller_idx)
            .filter_map(|idx| self.graph.node_weight(idx).cloned())
            .collect()
    }

    pub fn get_transitive_callees(&self, caller: &FunctionId) -> Vec<FunctionId> {
        let Some(&caller_idx) = self.node_map.get(caller) else {
            return Vec::new();
        };

        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut stack = vec![caller_idx];

        while let Some(idx) = stack.pop() {
            for neighbor_idx in self.graph.neighbors(idx) {
                if visited.insert(neighbor_idx) {
                    if let Some(id) = self.graph.node_weight(neighbor_idx) {
                        result.push(id.clone());
                    }
                    stack.push(neighbor_idx);
                }
            }
        }

        result
    }
}

impl Default for CallGraph {
    fn default() -> Self {
        Self::new()
    }
}

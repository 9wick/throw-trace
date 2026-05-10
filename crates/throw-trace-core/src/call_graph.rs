use crate::FunctionId;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

pub struct CallGraph {
    graph: DiGraph<FunctionId, ()>,
    node_map: HashMap<FunctionId, NodeIndex>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, id: FunctionId) {
        if !self.node_map.contains_key(&id) {
            let idx = self.graph.add_node(id.clone());
            self.node_map.insert(id, idx);
        }
    }

    pub fn contains(&self, id: &FunctionId) -> bool {
        self.node_map.contains_key(id)
    }

    pub fn add_call(&mut self, caller: &FunctionId, callee: &FunctionId) {
        if let (Some(&caller_idx), Some(&callee_idx)) =
            (self.node_map.get(caller), self.node_map.get(callee))
        {
            self.graph.add_edge(caller_idx, callee_idx, ());
        }
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

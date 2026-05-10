use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use throw_trace_core::{
    generate_diagnostics, CallGraph, Diagnostic, FunctionId, FunctionSignature,
};
use throw_trace_ts::extract_functions;

pub struct Analyzer {
    signatures: HashMap<FunctionId, FunctionSignature>,
    graph: CallGraph,
}

impl Analyzer {
    pub fn new() -> Self {
        Self { signatures: HashMap::new(), graph: CallGraph::new() }
    }

    pub fn analyze_files(&mut self, files: &[PathBuf]) -> Result<()> {
        for file in files {
            self.analyze_file(file)?;
        }
        self.build_call_graph();
        Ok(())
    }

    fn analyze_file(&mut self, path: &PathBuf) -> Result<()> {
        let source = fs::read_to_string(path)?;
        let sigs = extract_functions(&source, path)?;

        for sig in sigs {
            self.graph.add_function(sig.id.clone());
            self.signatures.insert(sig.id.clone(), sig);
        }

        Ok(())
    }

    fn build_call_graph(&mut self) {
        let sig_map: HashMap<&str, &FunctionId> =
            self.signatures.values().map(|sig| (sig.id.name.as_str(), &sig.id)).collect();

        let calls_to_add: Vec<(FunctionId, FunctionId)> = self
            .signatures
            .values()
            .flat_map(|sig| {
                sig.calls.iter().filter_map(|call| {
                    sig_map
                        .get(call.callee_name.as_str())
                        .map(|callee_id| (sig.id.clone(), (*callee_id).clone()))
                })
            })
            .collect();

        for (caller, callee) in calls_to_add {
            self.graph.add_call(&caller, &callee);
        }
    }

    pub fn generate_diagnostics(&self) -> Vec<Diagnostic> {
        generate_diagnostics(&self.signatures, &self.graph)
    }
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

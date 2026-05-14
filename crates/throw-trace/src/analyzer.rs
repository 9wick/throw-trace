use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use throw_trace_core::{
    generate_diagnostics_with_resolver, generate_lsp_violations, CallGraph, Diagnostic, FunctionId,
    FunctionSignature, LspViolation, MethodSignature, Span, TypeRelation,
};
use throw_trace_ts::{byte_offset_to_line_col, extract_all, TsServer, TsServerTypeResolver};

pub struct AnalyzerConfig {
    pub max_depth: u32,
    pub max_files: usize,
    pub cross_file: bool,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self { max_depth: 10, max_files: 1000, cross_file: true }
    }
}

pub struct Analyzer {
    signatures: HashMap<FunctionId, FunctionSignature>,
    method_signatures: Vec<MethodSignature>,
    type_relations: Vec<TypeRelation>,
    graph: CallGraph,
    entry_files: HashSet<PathBuf>,
    analyzed_files: HashSet<PathBuf>,
    config: AnalyzerConfig,
    /// Maps (caller_file, callee_span) -> (def_file, def_line) for resolved definitions
    resolved_calls: HashMap<(PathBuf, Span), (PathBuf, u32)>,
}

impl Analyzer {
    pub fn new() -> Self {
        Self::with_config(AnalyzerConfig::default())
    }

    pub fn with_config(config: AnalyzerConfig) -> Self {
        Self {
            signatures: HashMap::new(),
            method_signatures: Vec::new(),
            type_relations: Vec::new(),
            graph: CallGraph::new(),
            entry_files: HashSet::new(),
            analyzed_files: HashSet::new(),
            config,
            resolved_calls: HashMap::new(),
        }
    }

    pub fn analyze_files(&mut self, files: &[PathBuf]) -> Result<()> {
        for file in files {
            let canonical = file.canonicalize().unwrap_or_else(|_| file.clone());
            self.entry_files.insert(canonical.clone());
            self.analyze_file(&canonical)?;
        }

        if self.config.cross_file {
            self.resolve_cross_file_calls()?;
        }

        self.build_call_graph();
        Ok(())
    }

    fn analyze_file(&mut self, path: &PathBuf) -> Result<()> {
        if self.analyzed_files.contains(path) {
            return Ok(());
        }

        if self.analyzed_files.len() >= self.config.max_files {
            return Ok(());
        }

        let source = fs::read_to_string(path)?;
        let result = extract_all(&source, path)?;

        for sig in result.signatures {
            self.graph.add_function(sig.id.clone());
            self.signatures.insert(sig.id.clone(), sig);
        }

        self.method_signatures.extend(result.method_signatures);
        self.type_relations.extend(result.type_relations);

        self.analyzed_files.insert(path.clone());
        Ok(())
    }

    fn resolve_cross_file_calls(&mut self) -> Result<()> {
        let ts_server = TsServer::new();
        if ts_server.is_err() {
            return Ok(());
        }
        let mut ts_server = ts_server.unwrap();

        let mut pending: VecDeque<PathBuf> = VecDeque::new();
        let mut depth = 0;

        loop {
            if depth >= self.config.max_depth {
                break;
            }

            let new_files = self.collect_definition_targets(&mut ts_server)?;
            if new_files.is_empty() {
                break;
            }

            for file in new_files {
                if !self.analyzed_files.contains(&file) && self.should_analyze(&file) {
                    pending.push_back(file);
                }
            }

            while let Some(file) = pending.pop_front() {
                if self.analyzed_files.len() >= self.config.max_files {
                    break;
                }
                let _ = self.analyze_file(&file);
            }

            depth += 1;
        }

        Ok(())
    }

    fn collect_definition_targets(&mut self, ts_server: &mut TsServer) -> Result<HashSet<PathBuf>> {
        let mut targets = HashSet::new();

        let call_infos: Vec<(PathBuf, Span)> = self
            .signatures
            .values()
            .flat_map(|sig| {
                sig.calls.iter().map(|call| (sig.id.file_path.clone(), call.callee_span))
            })
            .collect();

        for (file_path, callee_span) in call_infos {
            if self.resolved_calls.contains_key(&(file_path.clone(), callee_span)) {
                continue;
            }

            let source = match fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let _ = ts_server.open_file(&file_path);

            let (line, col) = byte_offset_to_line_col(&source, callee_span.start);

            if let Ok(definitions) = ts_server.definition(&file_path, line, col) {
                if let Some(def) = definitions.first() {
                    let def_path = PathBuf::from(&def.file);
                    let canonical = def_path.canonicalize().unwrap_or_else(|_| def_path.clone());

                    self.resolved_calls.insert(
                        (file_path.clone(), callee_span),
                        (canonical.clone(), def.start.line),
                    );

                    if !self.analyzed_files.contains(&canonical) {
                        targets.insert(canonical);
                    }
                }
            }
        }

        Ok(targets)
    }

    fn should_analyze(&self, file_path: &Path) -> bool {
        !file_path.components().any(|c| c.as_os_str() == "node_modules")
    }

    fn build_call_graph(&mut self) {
        let mut sig_by_file_and_line: HashMap<(PathBuf, u32), &FunctionId> = HashMap::new();
        let mut sig_by_name: HashMap<&str, Vec<&FunctionId>> = HashMap::new();

        for sig in self.signatures.values() {
            let source = fs::read_to_string(&sig.id.file_path).unwrap_or_default();
            let (line, _) = byte_offset_to_line_col(&source, sig.name_span.start);
            sig_by_file_and_line.insert((sig.id.file_path.clone(), line), &sig.id);
            sig_by_name.entry(sig.id.name.as_str()).or_default().push(&sig.id);
        }

        let calls_to_add: Vec<(FunctionId, FunctionId)> = self
            .signatures
            .values()
            .flat_map(|sig| {
                sig.calls.iter().filter_map(|call| {
                    let key = (sig.id.file_path.clone(), call.callee_span);
                    if let Some((def_file, def_line)) = self.resolved_calls.get(&key) {
                        if let Some(callee_id) =
                            sig_by_file_and_line.get(&(def_file.clone(), *def_line))
                        {
                            return Some((sig.id.clone(), (*callee_id).clone()));
                        }
                    }

                    let candidates = sig_by_name.get(call.callee_name.as_str())?;
                    candidates
                        .iter()
                        .find(|c| c.file_path == sig.id.file_path)
                        .map(|callee_id| (sig.id.clone(), (*callee_id).clone()))
                })
            })
            .collect();

        for (caller, callee) in calls_to_add {
            self.graph.add_call(&caller, &callee);
        }
    }

    pub fn generate_diagnostics(&self) -> Vec<Diagnostic> {
        let all_diagnostics = if let Ok(mut resolver) = TsServerTypeResolver::new() {
            generate_diagnostics_with_resolver(&self.signatures, &self.graph, &mut resolver)
        } else {
            eprintln!("warning: tsserver not available, falling back to string comparison");
            generate_diagnostics_with_resolver(
                &self.signatures,
                &self.graph,
                &mut throw_trace_core::NoOpTypeResolver,
            )
        };

        all_diagnostics
            .into_iter()
            .filter(|d| self.entry_files.contains(&d.function.file_path))
            .collect()
    }

    pub fn generate_lsp_violations(&self) -> Vec<LspViolation> {
        let all_violations = if let Ok(mut resolver) = TsServerTypeResolver::new() {
            generate_lsp_violations(
                &self.signatures,
                &self.method_signatures,
                &self.type_relations,
                &self.graph,
                &mut resolver,
            )
        } else {
            generate_lsp_violations(
                &self.signatures,
                &self.method_signatures,
                &self.type_relations,
                &self.graph,
                &mut throw_trace_core::NoOpTypeResolver,
            )
        };

        all_violations
            .into_iter()
            .filter(|v| self.entry_files.contains(&v.implementation.file_path))
            .collect()
    }
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

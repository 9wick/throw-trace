use crate::cache::{
    resolution_fingerprint_entries_for_files, CacheStore, CachedExtraction, CachedTypeResolver,
};
use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use throw_trace_core::{
    compute_propagated_throws, find_missing_declarations, generate_lsp_violations, CallGraph,
    Diagnostic, FunctionId, FunctionSignature, LspViolation, MethodSignature, Span, TypeRelation,
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
    /// Maps `(caller_file, callee_span)` -> `(def_file, def_line)` for resolved definitions
    resolved_calls: HashMap<(PathBuf, Span), (PathBuf, u32)>,
    cache: CacheStore,
}

impl Analyzer {
    pub fn new() -> Self {
        Self::with_config(AnalyzerConfig::default())
    }

    pub fn with_config(config: AnalyzerConfig) -> Self {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workspace_fingerprint = initial_workspace_fingerprint(&config);

        Self {
            signatures: HashMap::new(),
            method_signatures: Vec::new(),
            type_relations: Vec::new(),
            graph: CallGraph::new(),
            entry_files: HashSet::new(),
            analyzed_files: HashSet::new(),
            config,
            resolved_calls: HashMap::new(),
            cache: CacheStore::load_deferred_workspace_validation(
                &workspace_root,
                workspace_fingerprint,
            ),
        }
    }

    pub fn analyze_files(&mut self, files: &[PathBuf]) -> Result<()> {
        let canonical_files: Vec<PathBuf> =
            files.iter().map(|file| file.canonicalize().unwrap_or_else(|_| file.clone())).collect();
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workspace_fingerprint =
            workspace_fingerprint_for_files(&workspace_root, &self.config, &canonical_files);
        self.cache.update_workspace_fingerprint(workspace_fingerprint);

        for canonical in canonical_files {
            self.entry_files.insert(canonical.clone());
            self.analyze_file(&canonical)?;
        }

        if self.config.cross_file {
            self.resolve_cross_file_calls();
        }

        self.build_call_graph();
        let _ = self.cache.save();
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
        let content_hash = CacheStore::content_hash(&source);

        let extraction = if let Some(cached) = self.cache.lookup_extraction(path, &content_hash) {
            cached
        } else {
            let result = extract_all(&source, path)?;
            let extraction = CachedExtraction::from_parts(
                result.signatures,
                result.method_signatures,
                result.type_relations,
            );
            self.cache.update_extraction(path, content_hash, extraction.clone());
            extraction
        };

        for sig in extraction.signatures {
            self.graph.add_function(sig.id.clone());
            self.signatures.insert(sig.id.clone(), sig);
        }

        self.method_signatures.extend(extraction.method_signatures);
        self.type_relations.extend(extraction.type_relations);

        self.analyzed_files.insert(path.clone());
        Ok(())
    }

    fn resolve_cross_file_calls(&mut self) {
        let Ok(mut ts_server) = TsServer::new() else {
            return;
        };

        let mut pending: VecDeque<PathBuf> = VecDeque::new();
        let mut depth = 0;

        loop {
            if depth >= self.config.max_depth {
                break;
            }

            let new_files = self.collect_definition_targets(&mut ts_server);
            if new_files.is_empty() {
                break;
            }

            for file in new_files {
                if !self.analyzed_files.contains(&file) && Self::should_analyze(&file) {
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
    }

    fn collect_definition_targets(&mut self, ts_server: &mut TsServer) -> HashSet<PathBuf> {
        let mut targets = HashSet::new();
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

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

            let Ok(source) = fs::read_to_string(&file_path) else {
                continue;
            };

            let content_hash = CacheStore::content_hash(&source);
            let Some(callee_text) =
                source.get(callee_span.start as usize..callee_span.end as usize)
            else {
                continue;
            };

            if let Some((def_file, def_line)) = self.cache.lookup_definition(
                &workspace_root,
                &file_path,
                &content_hash,
                callee_span,
                callee_text,
            ) {
                self.resolved_calls
                    .insert((file_path.clone(), callee_span), (def_file.clone(), def_line));
                if !self.analyzed_files.contains(&def_file) {
                    targets.insert(def_file);
                }
                continue;
            }

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

                    self.cache.update_definition(
                        &workspace_root,
                        &file_path,
                        content_hash,
                        callee_span,
                        callee_text.to_string(),
                        (canonical.clone(), def.start.line),
                    );

                    if !self.analyzed_files.contains(&canonical) {
                        targets.insert(canonical);
                    }
                }
            }
        }

        targets
    }

    fn should_analyze(file_path: &Path) -> bool {
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

        let calls_to_add: Vec<(FunctionId, FunctionId, Span)> = self
            .signatures
            .values()
            .flat_map(|sig| {
                sig.calls.iter().filter_map(|call| {
                    let key = (sig.id.file_path.clone(), call.callee_span);
                    if let Some((def_file, def_line)) = self.resolved_calls.get(&key) {
                        if let Some(callee_id) =
                            sig_by_file_and_line.get(&(def_file.clone(), *def_line))
                        {
                            return Some((sig.id.clone(), (*callee_id).clone(), call.location));
                        }
                    }

                    let candidates = sig_by_name.get(call.callee_name.as_str())?;
                    candidates
                        .iter()
                        .find(|c| c.file_path == sig.id.file_path)
                        .map(|callee_id| (sig.id.clone(), (*callee_id).clone(), call.location))
                })
            })
            .collect();

        for (caller, callee, location) in calls_to_add {
            self.graph.add_call_with_location(&caller, &callee, location);
        }
    }

    pub fn generate_diagnostics(&mut self) -> Vec<Diagnostic> {
        let function_ids = self.sorted_function_ids();

        let all_diagnostics = if let Ok(resolver) = TsServerTypeResolver::new() {
            let fingerprints = self.dependency_fingerprints(&function_ids, "tsserver");
            let scope_fingerprint = self.type_environment_fingerprint();
            let mut resolver =
                CachedTypeResolver::new(&mut self.cache, resolver, scope_fingerprint);
            let mut diagnostics = Vec::new();

            for func_id in function_ids {
                let Some(sig) = self.signatures.get(&func_id) else {
                    continue;
                };
                let Some(fingerprint) = fingerprints.get(&func_id) else {
                    continue;
                };

                if let Some(cached) = resolver.cache_mut().lookup_diagnostic(&func_id, fingerprint)
                {
                    if let Some(diagnostic) = cached {
                        diagnostics.push(diagnostic);
                    }
                    continue;
                }

                let propagated = compute_propagated_throws(&func_id, &self.signatures, &self.graph);
                resolver.take_recorded();
                let missing = find_missing_declarations(sig, &propagated, &mut resolver);
                let type_check_dependencies = resolver.take_recorded();
                let diagnostic = if missing.is_empty() {
                    None
                } else {
                    Some(Diagnostic { function: func_id.clone(), missing_throws: missing })
                };
                resolver.cache_mut().update_diagnostic(
                    &func_id,
                    fingerprint.clone(),
                    type_check_dependencies,
                    diagnostic.clone(),
                );
                if let Some(diagnostic) = diagnostic {
                    diagnostics.push(diagnostic);
                }
            }

            diagnostics
        } else {
            eprintln!("warning: tsserver not available, falling back to string comparison");
            let fingerprints = self.dependency_fingerprints(&function_ids, "noop");
            let mut resolver = throw_trace_core::NoOpTypeResolver;
            let mut diagnostics = Vec::new();

            for func_id in function_ids {
                let Some(sig) = self.signatures.get(&func_id) else {
                    continue;
                };
                let Some(fingerprint) = fingerprints.get(&func_id) else {
                    continue;
                };

                if let Some(cached) = self.cache.lookup_diagnostic(&func_id, fingerprint) {
                    if let Some(diagnostic) = cached {
                        diagnostics.push(diagnostic);
                    }
                    continue;
                }

                let propagated = compute_propagated_throws(&func_id, &self.signatures, &self.graph);
                let missing = find_missing_declarations(sig, &propagated, &mut resolver);
                let diagnostic = if missing.is_empty() {
                    None
                } else {
                    Some(Diagnostic { function: func_id.clone(), missing_throws: missing })
                };
                self.cache.update_diagnostic(
                    &func_id,
                    fingerprint.clone(),
                    Vec::new(),
                    diagnostic.clone(),
                );
                if let Some(diagnostic) = diagnostic {
                    diagnostics.push(diagnostic);
                }
            }

            diagnostics
        };
        let _ = self.cache.save();

        all_diagnostics
            .into_iter()
            .filter(|d| self.entry_files.contains(&d.function.file_path))
            .collect()
    }

    fn sorted_function_ids(&self) -> Vec<FunctionId> {
        let mut function_ids: Vec<FunctionId> = self.signatures.keys().cloned().collect();
        function_ids.sort_by_key(function_id_sort_key);
        function_ids
    }

    fn dependency_fingerprints(
        &self,
        function_ids: &[FunctionId],
        resolver_mode: &str,
    ) -> HashMap<FunctionId, String> {
        // 全関数で共通の部分は一度だけ計算する
        let shared_parts: Vec<(String, String)> = vec![
            ("resolver_mode".to_string(), resolver_mode.to_string()),
            ("workspace_fingerprint".to_string(), self.cache.workspace_fingerprint().to_string()),
            ("method_signatures".to_string(), self.method_signatures_hash()),
            ("type_relations".to_string(), self.type_relations_hash()),
        ];

        function_ids
            .iter()
            .map(|func_id| (func_id.clone(), self.dependency_fingerprint(func_id, &shared_parts)))
            .collect()
    }

    // 関数自身と推移的 callee の signature・call edge にのみ依存させる。
    // ワークスペース全体のソースハッシュ等を混ぜると無関係な変更で全 miss する
    fn dependency_fingerprint(
        &self,
        func_id: &FunctionId,
        shared_parts: &[(String, String)],
    ) -> String {
        let mut parts: Vec<(String, String)> = shared_parts.to_vec();

        if let Some(sig) = self.signatures.get(func_id) {
            parts.push(("function_signature".to_string(), CacheStore::stable_json_hash(sig)));
        }

        for callee in self.sorted_transitive_callees(func_id) {
            if let Some(sig) = self.signatures.get(&callee) {
                parts.push(("callee_signature".to_string(), CacheStore::stable_json_hash(sig)));
            }
        }

        for edge in self.sorted_reachable_call_edges(func_id) {
            parts.push(("call_edge".to_string(), CacheStore::stable_json_hash(&edge)));
        }

        CacheStore::stable_json_hash(&parts)
    }

    fn method_signatures_hash(&self) -> String {
        let mut methods = self.method_signatures.clone();
        methods.sort_by_key(method_signature_sort_key);
        CacheStore::stable_json_hash(&methods)
    }

    fn type_relations_hash(&self) -> String {
        let mut relations = self.type_relations.clone();
        relations.sort_by_key(type_relation_sort_key);
        CacheStore::stable_json_hash(&relations)
    }

    // 型チェック結果の有効範囲: 解析対象の型階層 (type_relations) と
    // 解決設定 (workspace_fingerprint) が変わらない限り再利用できる
    fn type_environment_fingerprint(&self) -> String {
        CacheStore::stable_json_hash(&(
            self.cache.workspace_fingerprint(),
            self.type_relations_hash(),
        ))
    }

    fn sorted_transitive_callees(&self, func_id: &FunctionId) -> Vec<FunctionId> {
        let mut callees = self.graph.get_transitive_callees(func_id);
        callees.sort_by_key(function_id_sort_key);
        callees
    }

    fn sorted_reachable_call_edges(
        &self,
        func_id: &FunctionId,
    ) -> Vec<(FunctionId, FunctionId, Vec<Span>)> {
        let mut visited = HashSet::new();
        let mut stack = vec![func_id.clone()];
        let mut edges = Vec::new();

        while let Some(caller) = stack.pop() {
            let mut callees = self.graph.get_callees(&caller);
            callees.sort_by_key(function_id_sort_key);

            for callee in callees {
                let mut locations = self.graph.get_call_site_locations(&caller, &callee).to_vec();
                locations.sort_by_key(|span| (span.start, span.end));
                edges.push((caller.clone(), callee.clone(), locations));

                if visited.insert(callee.clone()) {
                    stack.push(callee);
                }
            }
        }

        edges.sort_by(|a, b| {
            (function_id_sort_key(&a.0), function_id_sort_key(&a.1), span_list_sort_key(&a.2)).cmp(
                &(function_id_sort_key(&b.0), function_id_sort_key(&b.1), span_list_sort_key(&b.2)),
            )
        });
        edges
    }

    pub fn generate_lsp_violations(&mut self) -> Vec<LspViolation> {
        let all_violations = if let Ok(resolver) = TsServerTypeResolver::new() {
            let scope_fingerprint = self.type_environment_fingerprint();
            let mut resolver =
                CachedTypeResolver::new(&mut self.cache, resolver, scope_fingerprint);
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
        let _ = self.cache.save();

        all_violations
            .into_iter()
            .filter(|v| self.entry_files.contains(&v.implementation.file_path))
            .collect()
    }
}

fn function_id_sort_key(id: &FunctionId) -> (String, String, u32, u32) {
    (path_sort_key(&id.file_path), id.name.to_string(), id.span.start, id.span.end)
}

fn method_signature_sort_key(
    method: &MethodSignature,
) -> (String, String, u32, u32, String, u32, u32) {
    (
        path_sort_key(&method.type_id.file_path),
        method.type_id.name.to_string(),
        method.type_id.span.start,
        method.type_id.span.end,
        method.method_name.to_string(),
        method.method_span.start,
        method.method_span.end,
    )
}

fn type_relation_sort_key(
    relation: &TypeRelation,
) -> (String, String, u32, u32, String, String, u32, u32, String) {
    (
        path_sort_key(&relation.child.file_path),
        relation.child.name.to_string(),
        relation.child.span.start,
        relation.child.span.end,
        path_sort_key(&relation.parent.file_path),
        relation.parent.name.to_string(),
        relation.parent.span.start,
        relation.parent.span.end,
        format!("{:?}", relation.kind),
    )
}

fn span_list_sort_key(spans: &[Span]) -> Vec<(u32, u32)> {
    spans.iter().map(|span| (span.start, span.end)).collect()
}

fn path_sort_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
fn workspace_fingerprint(workspace_root: &Path, config: &AnalyzerConfig) -> String {
    workspace_fingerprint_for_files(workspace_root, config, &[])
}

fn initial_workspace_fingerprint(config: &AnalyzerConfig) -> String {
    CacheStore::stable_json_hash(&(
        env!("CARGO_PKG_VERSION"),
        config.max_depth,
        config.max_files,
        config.cross_file,
        "initial-without-resolution-scan",
    ))
}

fn workspace_fingerprint_for_files(
    workspace_root: &Path,
    config: &AnalyzerConfig,
    files: &[PathBuf],
) -> String {
    CacheStore::stable_json_hash(&(
        env!("CARGO_PKG_VERSION"),
        config.max_depth,
        config.max_files,
        config.cross_file,
        resolution_fingerprint_entries_for_files(workspace_root, files),
    ))
}

#[cfg(test)]
fn workspace_resolution_fingerprint_entries(workspace_root: &Path) -> Vec<(String, String)> {
    resolution_fingerprint_entries_for_files(workspace_root, &[])
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_tsconfig_is_included_in_workspace_resolution_fingerprint_entries() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let nested_dir = temp_dir.path().join("packages/foo");
        fs::create_dir_all(&nested_dir).expect("nested dir should be created");
        fs::write(nested_dir.join("tsconfig.json"), "{}").expect("tsconfig should be written");

        let entries = workspace_resolution_fingerprint_entries(temp_dir.path());

        assert!(entries.iter().any(|(path, _)| path == "packages/foo/tsconfig.json"));
    }

    #[test]
    fn ignored_directories_are_excluded_from_workspace_resolution_fingerprint_entries() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        fs::create_dir_all(temp_dir.path().join("node_modules/pkg"))
            .expect("node_modules dir should be created");
        fs::create_dir_all(temp_dir.path().join(".git")).expect(".git dir should be created");
        fs::create_dir_all(temp_dir.path().join(".throw-trace"))
            .expect(".throw-trace dir should be created");
        fs::write(temp_dir.path().join("node_modules/pkg/package.json"), "{}")
            .expect("node_modules package should be written");
        fs::write(temp_dir.path().join(".git/config"), "[core]")
            .expect("git config should be written");
        fs::write(temp_dir.path().join(".throw-trace/cache.json"), "{}")
            .expect("cache should be written");

        let entries = workspace_resolution_fingerprint_entries(temp_dir.path());

        assert!(entries.is_empty());
    }

    #[test]
    fn changing_nested_tsconfig_changes_workspace_fingerprint() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let nested_dir = temp_dir.path().join("packages/foo");
        fs::create_dir_all(&nested_dir).expect("nested dir should be created");
        let tsconfig = nested_dir.join("tsconfig.json");
        fs::write(&tsconfig, r#"{"compilerOptions":{}}"#).expect("tsconfig should be written");
        let config = AnalyzerConfig::default();

        let first = workspace_fingerprint(temp_dir.path(), &config);
        fs::write(&tsconfig, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("tsconfig should be updated");
        let second = workspace_fingerprint(temp_dir.path(), &config);

        assert_ne!(first, second);
    }

    #[test]
    fn external_analyzed_file_ancestor_tsconfig_changes_workspace_fingerprint() {
        let workspace_dir = tempfile::tempdir().expect("workspace tempdir should be created");
        let external_dir = tempfile::tempdir().expect("external tempdir should be created");
        let external_src = external_dir.path().join("external/src");
        fs::create_dir_all(&external_src).expect("external src should be created");
        let tsconfig = external_dir.path().join("external/tsconfig.json");
        let source_file = external_src.join("a.ts");
        fs::write(&tsconfig, r#"{"compilerOptions":{}}"#).expect("tsconfig should be written");
        fs::write(&source_file, "export const a = 1;").expect("source should be written");
        let config = AnalyzerConfig::default();

        let first = workspace_fingerprint_for_files(
            workspace_dir.path(),
            &config,
            std::slice::from_ref(&source_file),
        );
        fs::write(&tsconfig, r#"{"compilerOptions":{"paths":{"@/*":["src/*"]}}}"#)
            .expect("tsconfig should be updated");
        let second = workspace_fingerprint_for_files(workspace_dir.path(), &config, &[source_file]);

        assert_ne!(first, second);
    }

    #[test]
    fn generated_directories_are_not_walked_for_workspace_resolution_fingerprint_entries() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        for (dir, file) in [
            ("target", "tsconfig.json"),
            ("dist", "package.json"),
            ("build", "tsconfig.json"),
            (".next", "package.json"),
            ("coverage", "package.json"),
            (".turbo", "package.json"),
        ] {
            let generated_dir = temp_dir.path().join(dir);
            fs::create_dir_all(&generated_dir).expect("generated dir should be created");
            fs::write(generated_dir.join(file), "{}").expect("generated file should be written");
        }

        let entries = workspace_resolution_fingerprint_entries(temp_dir.path());

        assert!(entries.is_empty());
    }
}

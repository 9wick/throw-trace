use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use throw_trace_core::{
    Diagnostic, FunctionId, FunctionSignature, MethodSignature, Span, TypeRelation, TypeResolver,
};

// Skeleton APIs are wired into Analyzer in later persistent-cache tasks.
#[allow(dead_code)]
pub const CACHE_SCHEMA_VERSION: u32 = 1;

// Skeleton APIs are wired into Analyzer in later persistent-cache tasks.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    pub schema_version: u32,
    pub tool_version: String,
    pub workspace_fingerprint: String,
    pub files: BTreeMap<PathBuf, CachedFile>,
    pub definitions: BTreeMap<String, CachedDefinition>,
    pub diagnostics: BTreeMap<String, CachedDiagnostic>,
    pub type_checks: BTreeMap<String, bool>,
}

impl CacheFile {
    pub fn new(workspace_fingerprint: String) -> Self {
        Self {
            schema_version: CACHE_SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            workspace_fingerprint,
            files: BTreeMap::new(),
            definitions: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            type_checks: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFile {
    pub content_hash: String,
    pub extraction: CachedExtraction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedExtraction {
    pub signatures: Vec<FunctionSignature>,
    pub method_signatures: Vec<MethodSignature>,
    pub type_relations: Vec<TypeRelation>,
}

impl CachedExtraction {
    pub fn from_parts(
        signatures: Vec<FunctionSignature>,
        method_signatures: Vec<MethodSignature>,
        type_relations: Vec<TypeRelation>,
    ) -> Self {
        Self { signatures, method_signatures, type_relations }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDefinition {
    pub caller_file: PathBuf,
    pub caller_hash: String,
    pub callee_span: Span,
    pub callee_text: String,
    pub definition_file: PathBuf,
    #[serde(default)]
    pub definition_file_hash: String,
    #[serde(default)]
    pub resolution_fingerprint: String,
    pub definition_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDiagnostic {
    pub dependency_fingerprint: String,
    // この診断を計算した際に参照した型チェックとその対象ファイル。
    // 対象ファイルが変わると型チェック結果も変わり得るため、lookup 時に検証する
    #[serde(default)]
    pub type_check_dependencies: Vec<TypeCheckDependency>,
    pub diagnostic: Option<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeCheckDependency {
    pub file: PathBuf,
    pub file_hash: String,
    pub thrown: String,
    pub declared: String,
    pub result: bool,
}

// Skeleton APIs are wired into Analyzer in later persistent-cache tasks.
#[allow(dead_code)]
#[derive(Debug)]
pub struct CacheStore {
    path: PathBuf,
    workspace_root: PathBuf,
    data: CacheFile,
    dirty: bool,
    workspace_resolution_fingerprint_cache: Option<(PathBuf, String)>,
}

// Skeleton APIs are wired into Analyzer in later persistent-cache tasks.
#[allow(dead_code)]
impl CacheStore {
    pub fn load(workspace_root: &Path, workspace_fingerprint: String) -> Self {
        let path = workspace_root.join(".throw-trace").join("cache.json");
        let workspace_root = canonical_or_self(workspace_root);
        let loaded = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<CacheFile>(&raw).ok());

        let (data, dirty) = match loaded {
            Some(cache) if cache.schema_version == CACHE_SCHEMA_VERSION => {
                let metadata_changed = cache.workspace_fingerprint != workspace_fingerprint
                    || cache.tool_version != env!("CARGO_PKG_VERSION");

                if metadata_changed {
                    (
                        CacheFile {
                            workspace_fingerprint,
                            tool_version: env!("CARGO_PKG_VERSION").to_string(),
                            definitions: BTreeMap::new(),
                            diagnostics: BTreeMap::new(),
                            type_checks: BTreeMap::new(),
                            ..cache
                        },
                        true,
                    )
                } else {
                    (cache, false)
                }
            }
            _ => (CacheFile::new(workspace_fingerprint), false),
        };

        Self { path, workspace_root, data, dirty, workspace_resolution_fingerprint_cache: None }
    }

    pub fn load_deferred_workspace_validation(
        workspace_root: &Path,
        initial_workspace_fingerprint: String,
    ) -> Self {
        let path = workspace_root.join(".throw-trace").join("cache.json");
        let workspace_root = canonical_or_self(workspace_root);
        let loaded = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<CacheFile>(&raw).ok());

        let (data, dirty) = match loaded {
            Some(cache) if cache.schema_version == CACHE_SCHEMA_VERSION => {
                if cache.tool_version == env!("CARGO_PKG_VERSION") {
                    (cache, false)
                } else {
                    (
                        CacheFile {
                            workspace_fingerprint: initial_workspace_fingerprint,
                            tool_version: env!("CARGO_PKG_VERSION").to_string(),
                            definitions: BTreeMap::new(),
                            diagnostics: BTreeMap::new(),
                            type_checks: BTreeMap::new(),
                            ..cache
                        },
                        true,
                    )
                }
            }
            _ => (CacheFile::new(initial_workspace_fingerprint), false),
        };

        Self { path, workspace_root, data, dirty, workspace_resolution_fingerprint_cache: None }
    }

    pub fn lookup_extraction(
        &self,
        file_path: &Path,
        content_hash: &str,
    ) -> Option<CachedExtraction> {
        let key = canonical_or_self(file_path);
        let entry = self.data.files.get(&key)?;
        if entry.content_hash == content_hash {
            Some(entry.extraction.clone())
        } else {
            None
        }
    }

    pub fn update_extraction(
        &mut self,
        file_path: &Path,
        content_hash: String,
        extraction: CachedExtraction,
    ) {
        let key = canonical_or_self(file_path);
        self.data.files.insert(key, CachedFile { content_hash, extraction });
        self.dirty = true;
    }

    pub fn update_workspace_fingerprint(&mut self, workspace_fingerprint: String) {
        if self.data.workspace_fingerprint == workspace_fingerprint {
            return;
        }

        self.data.workspace_fingerprint = workspace_fingerprint;
        self.data.definitions.clear();
        self.data.diagnostics.clear();
        self.data.type_checks.clear();
        self.workspace_resolution_fingerprint_cache = None;
        self.dirty = true;
    }

    pub fn workspace_fingerprint(&self) -> &str {
        &self.data.workspace_fingerprint
    }

    #[allow(clippy::option_option)]
    pub fn lookup_diagnostic(
        &self,
        function_id: &FunctionId,
        dependency_fingerprint: &str,
    ) -> Option<Option<Diagnostic>> {
        let key = function_cache_key(function_id);
        let entry = self.data.diagnostics.get(&key)?;
        if entry.dependency_fingerprint != dependency_fingerprint {
            return None;
        }
        for dep in &entry.type_check_dependencies {
            let source = fs::read_to_string(&dep.file).ok()?;
            if Self::content_hash(&source) != dep.file_hash {
                return None;
            }
        }
        Some(entry.diagnostic.clone())
    }

    pub fn update_diagnostic(
        &mut self,
        function_id: &FunctionId,
        dependency_fingerprint: String,
        type_check_dependencies: Vec<TypeCheckDependency>,
        diagnostic: Option<Diagnostic>,
    ) {
        let key = function_cache_key(function_id);
        self.data.diagnostics.insert(
            key,
            CachedDiagnostic { dependency_fingerprint, type_check_dependencies, diagnostic },
        );
        self.dirty = true;
    }

    pub fn type_check_key(
        file_path: &Path,
        file_hash: &str,
        scope_fingerprint: &str,
        thrown_type: &str,
        declared_type: &str,
    ) -> String {
        let mut parts = BTreeMap::new();
        parts.insert("file", canonical_or_self(file_path).display().to_string());
        parts.insert("file_hash", file_hash.to_string());
        parts.insert("scope", scope_fingerprint.to_string());
        parts.insert("thrown", thrown_type.to_string());
        parts.insert("declared", declared_type.to_string());
        Self::stable_json_hash(&parts)
    }

    pub fn lookup_type_check(
        &self,
        file_path: &Path,
        file_hash: &str,
        scope_fingerprint: &str,
        thrown_type: &str,
        declared_type: &str,
    ) -> Option<bool> {
        let key = Self::type_check_key(
            file_path,
            file_hash,
            scope_fingerprint,
            thrown_type,
            declared_type,
        );
        self.data.type_checks.get(&key).copied()
    }

    pub fn update_type_check(
        &mut self,
        file_path: &Path,
        file_hash: &str,
        scope_fingerprint: &str,
        thrown_type: &str,
        declared_type: &str,
        result: bool,
    ) {
        let key = Self::type_check_key(
            file_path,
            file_hash,
            scope_fingerprint,
            thrown_type,
            declared_type,
        );
        self.data.type_checks.insert(key, result);
        self.dirty = true;
    }

    pub fn definition_key(
        file_path: &Path,
        content_hash: &str,
        callee_span: Span,
        callee_text: &str,
    ) -> String {
        let mut parts = BTreeMap::new();
        parts.insert("caller_file", canonical_or_self(file_path).display().to_string());
        parts.insert("caller_hash", content_hash.to_string());
        parts.insert("span_start", callee_span.start.to_string());
        parts.insert("span_end", callee_span.end.to_string());
        parts.insert("callee_text", callee_text.to_string());
        Self::stable_json_hash(&parts)
    }

    pub fn lookup_definition(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        content_hash: &str,
        callee_span: Span,
        callee_text: &str,
    ) -> Option<(PathBuf, u32)> {
        let key = Self::definition_key(file_path, content_hash, callee_span, callee_text);
        let entry = self.data.definitions.get(&key)?.clone();
        let definition_source = fs::read_to_string(&entry.definition_file).ok()?;
        let definition_file_hash = Self::content_hash(&definition_source);
        let resolution_fingerprint = self.resolution_fingerprint_for_files(
            workspace_root,
            &[entry.caller_file.clone(), entry.definition_file.clone()],
        );
        if entry.caller_hash == content_hash
            && entry.callee_span == callee_span
            && entry.callee_text == callee_text
            && entry.definition_file_hash == definition_file_hash
            && entry.resolution_fingerprint == resolution_fingerprint
        {
            Some((entry.definition_file.clone(), entry.definition_line))
        } else {
            None
        }
    }

    pub fn update_definition(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        content_hash: String,
        callee_span: Span,
        callee_text: String,
        definition: (PathBuf, u32),
    ) {
        let (definition_file, definition_line) = definition;
        let Ok(definition_source) = fs::read_to_string(&definition_file) else {
            return;
        };

        let key = Self::definition_key(file_path, &content_hash, callee_span, &callee_text);
        let caller_file = canonical_or_self(file_path);
        let resolution_fingerprint = self.resolution_fingerprint_for_files(
            workspace_root,
            &[caller_file.clone(), definition_file.clone()],
        );
        self.data.definitions.insert(
            key,
            CachedDefinition {
                caller_file,
                caller_hash: content_hash,
                callee_span,
                callee_text,
                definition_file_hash: Self::content_hash(&definition_source),
                resolution_fingerprint,
                definition_file,
                definition_line,
            },
        );
        self.dirty = true;
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
            let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer_pretty(&mut tmp, &self.data)?;
            tmp.persist(&self.path)?;
        }

        self.dirty = false;
        Ok(())
    }

    fn resolution_fingerprint_for_files(
        &mut self,
        workspace_root: &Path,
        files: &[PathBuf],
    ) -> String {
        let normalized_root = canonical_or_self(workspace_root);
        let normalized_root = if normalized_root == self.workspace_root {
            self.workspace_root.clone()
        } else {
            normalized_root
        };
        let workspace_fingerprint = self.workspace_resolution_fingerprint(&normalized_root);
        let file_entries = resolution_fingerprint_entries_for_files_with_workspace_entries(
            &normalized_root,
            files,
            BTreeMap::new(),
        );
        CacheStore::stable_json_hash(&(workspace_fingerprint, file_entries))
    }

    fn workspace_resolution_fingerprint(&mut self, workspace_root: &Path) -> String {
        let normalized_root = canonical_or_self(workspace_root);
        if let Some((cached_root, fingerprint)) = &self.workspace_resolution_fingerprint_cache {
            if cached_root == &normalized_root {
                return fingerprint.clone();
            }
        }

        let fingerprint = CacheStore::stable_json_hash(&workspace_resolution_fingerprint_entries(
            &normalized_root,
        ));
        self.workspace_resolution_fingerprint_cache = Some((normalized_root, fingerprint.clone()));
        fingerprint
    }

    pub fn content_hash(source: &str) -> String {
        hash_bytes(source.as_bytes())
    }

    pub fn stable_json_hash<T: Serialize>(value: &T) -> String {
        let bytes = serde_json::to_vec(value).expect("cache key serialization should not fail");
        hash_bytes(&bytes)
    }
}

pub struct CachedTypeResolver<'a, R> {
    cache: &'a mut CacheStore,
    inner: R,
    scope_fingerprint: String,
    // 直近の take_recorded 以降に参照した型チェック（診断の依存として保存する）
    recorded: Vec<TypeCheckDependency>,
}

impl<'a, R> CachedTypeResolver<'a, R> {
    pub fn new(cache: &'a mut CacheStore, inner: R, scope_fingerprint: String) -> Self {
        Self { cache, inner, scope_fingerprint, recorded: Vec::new() }
    }

    pub fn cache_mut(&mut self) -> &mut CacheStore {
        self.cache
    }

    pub fn take_recorded(&mut self) -> Vec<TypeCheckDependency> {
        std::mem::take(&mut self.recorded)
    }
}

impl<R: TypeResolver> TypeResolver for CachedTypeResolver<'_, R> {
    fn is_assignable_to(
        &mut self,
        file_path: &Path,
        thrown_type: &str,
        declared_type: &str,
    ) -> bool {
        let Ok(source) = fs::read_to_string(file_path) else {
            return self.inner.is_assignable_to(file_path, thrown_type, declared_type);
        };
        let file_hash = CacheStore::content_hash(&source);

        let result = if let Some(result) = self.cache.lookup_type_check(
            file_path,
            &file_hash,
            &self.scope_fingerprint,
            thrown_type,
            declared_type,
        ) {
            result
        } else {
            let result = self.inner.is_assignable_to(file_path, thrown_type, declared_type);
            self.cache.update_type_check(
                file_path,
                &file_hash,
                &self.scope_fingerprint,
                thrown_type,
                declared_type,
                result,
            );
            result
        };

        self.recorded.push(TypeCheckDependency {
            file: file_path.to_path_buf(),
            file_hash,
            thrown: thrown_type.to_string(),
            declared: declared_type.to_string(),
            result,
        });
        result
    }

    fn resolve_type(&mut self, file_path: &Path, span: Span) -> Option<String> {
        self.inner.resolve_type(file_path, span)
    }
}

// Skeleton APIs are wired into Analyzer in later persistent-cache tasks.
#[allow(dead_code)]
pub fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[allow(dead_code)]
pub fn resolution_fingerprint_for_files(workspace_root: &Path, files: &[PathBuf]) -> String {
    CacheStore::stable_json_hash(&resolution_fingerprint_entries_for_files(workspace_root, files))
}

pub(crate) fn resolution_fingerprint_entries_for_files(
    workspace_root: &Path,
    files: &[PathBuf],
) -> Vec<(String, String)> {
    resolution_fingerprint_entries_for_files_with_workspace_entries(
        workspace_root,
        files,
        workspace_resolution_fingerprint_entries(workspace_root),
    )
}

fn workspace_resolution_fingerprint_entries(workspace_root: &Path) -> BTreeMap<String, String> {
    let mut context = ResolutionFingerprintContext::new(workspace_root);
    collect_workspace_resolution_fingerprint_entries(workspace_root, &mut context);
    context.entries
}

fn resolution_fingerprint_entries_for_files_with_workspace_entries(
    workspace_root: &Path,
    files: &[PathBuf],
    workspace_entries: BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let mut context = ResolutionFingerprintContext::with_entries(workspace_root, workspace_entries);
    for file in files {
        collect_file_ancestor_resolution_fingerprint_entries(workspace_root, file, &mut context);
    }
    context.entries.into_iter().collect()
}

struct ResolutionFingerprintContext<'a> {
    workspace_root: &'a Path,
    entries: BTreeMap<String, String>,
    content_hashes: BTreeMap<PathBuf, String>,
    config_json_values: BTreeMap<PathBuf, Option<serde_json::Value>>,
    package_json_values: BTreeMap<PathBuf, Option<serde_json::Value>>,
    visited_configs: HashSet<PathBuf>,
}

impl<'a> ResolutionFingerprintContext<'a> {
    fn new(workspace_root: &'a Path) -> Self {
        Self::with_entries(workspace_root, BTreeMap::new())
    }

    fn with_entries(workspace_root: &'a Path, entries: BTreeMap<String, String>) -> Self {
        Self {
            workspace_root,
            entries,
            content_hashes: BTreeMap::new(),
            config_json_values: BTreeMap::new(),
            package_json_values: BTreeMap::new(),
            visited_configs: HashSet::new(),
        }
    }

    fn insert_file(&mut self, path: &Path) {
        self.insert_file_with_config_tracking(path, false);
    }

    fn insert_config_file(&mut self, path: &Path) {
        self.insert_file_with_config_tracking(path, true);
    }

    fn insert_file_with_config_tracking(&mut self, path: &Path, force_config_tracking: bool) {
        let normalized_path = normalized_path(path);
        let key = resolution_fingerprint_path_key(self.workspace_root, &normalized_path);
        let content_hash = self.content_hash(&normalized_path);
        self.entries.insert(key, content_hash);

        if force_config_tracking || is_config_reference_source(&normalized_path) {
            self.collect_config_references(&normalized_path);
        }
    }

    fn insert_marker(
        &mut self,
        source_path: &Path,
        reference_kind: &str,
        reference: &str,
        marker: &str,
    ) {
        let source_key = resolution_fingerprint_path_key(self.workspace_root, source_path);
        self.entries
            .insert(format!("{source_key}::{reference_kind}::{reference}"), marker.to_string());
    }

    fn content_hash(&mut self, path: &Path) -> String {
        if let Some(content_hash) = self.content_hashes.get(path) {
            return content_hash.clone();
        }

        let content_hash =
            fs::read(path).map_or_else(|_| "<unreadable>".to_string(), |bytes| hash_bytes(&bytes));
        self.content_hashes.insert(path.to_path_buf(), content_hash.clone());
        content_hash
    }

    fn config_json_value(&mut self, path: &Path) -> Option<serde_json::Value> {
        if let Some(value) = self.config_json_values.get(path) {
            return value.clone();
        }

        let value = fs::read_to_string(path).ok().and_then(|raw| {
            jsonc_parser::parse_to_serde_value(&raw, &jsonc_parser::ParseOptions::default())
                .ok()
                .flatten()
        });
        self.config_json_values.insert(path.to_path_buf(), value.clone());
        value
    }

    fn package_json_value(&mut self, path: &Path) -> Option<serde_json::Value> {
        if let Some(value) = self.package_json_values.get(path) {
            return value.clone();
        }

        let value = fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
        self.package_json_values.insert(path.to_path_buf(), value.clone());
        value
    }

    fn collect_config_references(&mut self, config_path: &Path) {
        if !self.visited_configs.insert(config_path.to_path_buf()) {
            return;
        }

        let Some(json) = self.config_json_value(config_path) else {
            return;
        };

        if let Some(extends) = json.get("extends") {
            match extends {
                serde_json::Value::String(reference) => {
                    self.collect_extends_reference(config_path, reference);
                }
                serde_json::Value::Array(references) => {
                    let references: Vec<Option<String>> = references
                        .iter()
                        .map(|reference| reference.as_str().map(str::to_string))
                        .collect();
                    for (index, reference) in references.into_iter().enumerate() {
                        if let Some(reference) = reference {
                            self.collect_extends_reference(config_path, &reference);
                        } else {
                            self.insert_marker(
                                config_path,
                                "extends",
                                &format!("<non-string:{index}>"),
                                "<invalid>",
                            );
                        }
                    }
                }
                _ => self.insert_marker(config_path, "extends", "<non-string>", "<invalid>"),
            }
        }

        if let Some(references) = json.get("references").and_then(|value| value.as_array()) {
            let paths: Vec<(usize, Option<String>, bool)> = references
                .iter()
                .enumerate()
                .map(|(index, reference)| {
                    let path = reference.get("path");
                    (index, path.and_then(|path| path.as_str()).map(str::to_string), path.is_some())
                })
                .collect();
            for (index, reference, has_path) in paths {
                if let Some(reference) = reference {
                    self.collect_project_reference(config_path, &reference);
                } else if has_path {
                    self.insert_marker(
                        config_path,
                        "references",
                        &format!("<non-string:{index}>"),
                        "<invalid>",
                    );
                } else {
                    self.insert_marker(
                        config_path,
                        "references",
                        &format!("<missing:{index}>"),
                        "<invalid>",
                    );
                }
            }
        }
    }

    fn collect_extends_reference(&mut self, config_path: &Path, reference: &str) {
        if is_package_style_reference(reference) {
            if let Some(referenced_path) =
                self.resolve_package_style_extends_path(config_path, reference)
            {
                self.insert_config_file(&referenced_path);
            } else {
                self.insert_marker(config_path, "extends", reference, "<package-ref>");
            }
            return;
        }

        let mut referenced_path = resolve_config_reference_path(config_path, reference);
        if referenced_path.extension().is_none() {
            referenced_path.set_extension("json");
        }

        if referenced_path.exists() {
            self.insert_config_file(&referenced_path);
        } else {
            self.insert_marker(config_path, "extends", reference, "<missing>");
        }
    }

    fn collect_project_reference(&mut self, config_path: &Path, reference: &str) {
        let referenced_path = resolve_config_reference_path(config_path, reference);
        let target = if referenced_path.is_dir() {
            referenced_path.join("tsconfig.json")
        } else {
            referenced_path
        };

        if target.exists() {
            self.insert_config_file(&target);
        } else {
            self.insert_marker(config_path, "references", reference, "<missing>");
        }
    }

    fn resolve_package_style_extends_path(
        &mut self,
        config_path: &Path,
        reference: &str,
    ) -> Option<PathBuf> {
        let mut current = config_path.parent();
        while let Some(dir) = current {
            let node_modules = dir.join("node_modules");
            for candidate in self.package_style_extends_candidates(&node_modules, reference) {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            current = dir.parent();
        }
        None
    }

    fn package_style_extends_candidates(
        &mut self,
        node_modules: &Path,
        reference: &str,
    ) -> Vec<PathBuf> {
        let reference_path = Path::new(reference);
        let mut candidates = Vec::new();
        if reference_path.extension().is_some() {
            candidates.push(node_modules.join(reference_path));
            return candidates;
        }

        candidates.push(node_modules.join(format!("{reference}.json")));
        if let Some(package_root) = package_style_package_root(reference) {
            let package_root_path = node_modules.join(package_root);
            let package_json = package_root_path.join("package.json");
            if package_json.exists() {
                self.insert_file(&package_json);
                if let Some(tsconfig) =
                    self.package_json_value(&normalized_path(&package_json)).and_then(|json| {
                        json.get("tsconfig").and_then(|value| value.as_str()).map(str::to_string)
                    })
                {
                    candidates.push(package_root_path.join(tsconfig));
                }
            }
        }
        candidates.push(node_modules.join(reference_path).join("tsconfig.json"));
        candidates
    }
}

fn collect_workspace_resolution_fingerprint_entries(
    dir: &Path,
    context: &mut ResolutionFingerprintContext<'_>,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            if is_excluded_resolution_dir(file_name) {
                continue;
            }
            collect_workspace_resolution_fingerprint_entries(&path, context);
        } else if file_type.is_file() && is_resolution_affecting_file(file_name) {
            context.insert_file(&path);
        }
    }
}

fn collect_file_ancestor_resolution_fingerprint_entries(
    workspace_root: &Path,
    file_path: &Path,
    context: &mut ResolutionFingerprintContext<'_>,
) {
    let mut current = file_path.parent();
    while let Some(dir) = current {
        collect_resolution_fingerprint_entries_in_dir(dir, context);
        if dir == workspace_root {
            break;
        }
        current = dir.parent();
    }
}

fn collect_resolution_fingerprint_entries_in_dir(
    dir: &Path,
    context: &mut ResolutionFingerprintContext<'_>,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() && is_resolution_affecting_file(file_name) {
            context.insert_file(&path);
        }
    }
}

fn is_excluded_resolution_dir(file_name: &str) -> bool {
    matches!(
        file_name,
        ".git"
            | ".throw-trace"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | "coverage"
            | ".turbo"
    )
}

fn resolution_fingerprint_path_key(workspace_root: &Path, path: &Path) -> String {
    if let Ok(relative_path) = path.strip_prefix(workspace_root) {
        relative_path.to_string_lossy().replace('\\', "/")
    } else {
        path.to_string_lossy().replace('\\', "/")
    }
}

fn normalized_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn resolve_config_reference_path(config_path: &Path, reference: &str) -> PathBuf {
    let reference_path = Path::new(reference);
    if reference_path.is_absolute() {
        reference_path.to_path_buf()
    } else {
        config_path.parent().unwrap_or_else(|| Path::new(".")).join(reference_path)
    }
}

fn is_package_style_reference(reference: &str) -> bool {
    let path = Path::new(reference);
    !path.is_absolute() && !reference.starts_with("./") && !reference.starts_with("../")
}

fn package_style_package_root(reference: &str) -> Option<PathBuf> {
    let mut parts = reference.split('/');
    let first = parts.next()?;
    if first.is_empty() {
        return None;
    }
    if first.starts_with('@') {
        let second = parts.next()?;
        if second.is_empty() {
            None
        } else {
            Some(PathBuf::from(first).join(second))
        }
    } else {
        Some(PathBuf::from(first))
    }
}

fn is_resolution_affecting_file(file_name: &str) -> bool {
    matches!(
        file_name,
        "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lockb"
            | "tsconfig.json"
            | "jsconfig.json"
    ) || (file_name.starts_with("tsconfig.") && has_json_extension(file_name))
        || (file_name.starts_with("jsconfig.") && has_json_extension(file_name))
}

fn has_json_extension(file_name: &str) -> bool {
    Path::new(file_name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
}

fn is_config_reference_source(path: &Path) -> bool {
    path.file_name().and_then(|file_name| file_name.to_str()).is_some_and(|file_name| {
        file_name == "tsconfig.json"
            || file_name == "jsconfig.json"
            || (file_name.starts_with("tsconfig.") && has_json_extension(file_name))
            || (file_name.starts_with("jsconfig.") && has_json_extension(file_name))
    })
}

// Skeleton APIs are wired into Analyzer in later persistent-cache tasks.
#[allow(dead_code)]
pub fn function_cache_key(id: &FunctionId) -> String {
    let mut parts = BTreeMap::new();
    parts.insert("file", canonical_or_self(&id.file_path).display().to_string());
    parts.insert("name", id.name.to_string());
    parts.insert("span_start", id.span.start.to_string());
    parts.insert("span_end", id.span.end.to_string());
    CacheStore::stable_json_hash(&parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached_file() -> CachedFile {
        CachedFile {
            content_hash: "old-hash".to_string(),
            extraction: CachedExtraction {
                signatures: Vec::new(),
                method_signatures: Vec::new(),
                type_relations: Vec::new(),
            },
        }
    }

    fn cache_with_file(workspace_fingerprint: &str) -> CacheFile {
        let mut cache = CacheFile::new(workspace_fingerprint.to_string());
        cache.files.insert(PathBuf::from("src/example.ts"), cached_file());
        cache
    }

    fn write_cache(root: &Path, cache: &CacheFile) {
        let cache_dir = root.join(".throw-trace");
        fs::create_dir_all(&cache_dir).expect("cache dir should be created");
        let payload = serde_json::to_string(cache).expect("cache should serialize");
        fs::write(cache_dir.join("cache.json"), payload).expect("cache file should be written");
    }

    #[test]
    fn content_hash_is_stable() {
        assert_eq!(CacheStore::content_hash("abc"), CacheStore::content_hash("abc"));
        assert_ne!(CacheStore::content_hash("abc"), CacheStore::content_hash("abcd"));
    }

    #[test]
    fn stable_json_hash_uses_btree_order() {
        let mut first = BTreeMap::new();
        first.insert("b", "2");
        first.insert("a", "1");

        let mut second = BTreeMap::new();
        second.insert("a", "1");
        second.insert("b", "2");

        assert_eq!(CacheStore::stable_json_hash(&first), CacheStore::stable_json_hash(&second));
    }

    #[test]
    fn definition_key_changes_when_callee_text_changes() {
        let path = Path::new("/tmp/a.ts");
        let span = Span { start: 10, end: 13 };
        let first = CacheStore::definition_key(path, "hash", span, "foo");
        let second = CacheStore::definition_key(path, "hash", span, "bar");
        assert_ne!(first, second);
    }

    #[test]
    fn type_check_key_includes_declared_type() {
        let path = Path::new("/tmp/a.ts");
        let first = CacheStore::type_check_key(path, "hash", "scope", "ChildError", "BaseError");
        let second = CacheStore::type_check_key(path, "hash", "scope", "ChildError", "OtherError");
        assert_ne!(first, second);
    }

    #[test]
    fn type_check_key_includes_file_hash() {
        let path = Path::new("/tmp/a.ts");
        let first =
            CacheStore::type_check_key(path, "first-hash", "scope", "ChildError", "BaseError");
        let second =
            CacheStore::type_check_key(path, "second-hash", "scope", "ChildError", "BaseError");
        assert_ne!(first, second);
    }

    #[test]
    fn type_check_key_includes_scope_fingerprint() {
        let path = Path::new("/tmp/a.ts");
        let first =
            CacheStore::type_check_key(path, "hash", "first-scope", "ChildError", "BaseError");
        let second =
            CacheStore::type_check_key(path, "hash", "second-scope", "ChildError", "BaseError");
        assert_ne!(first, second);
    }

    #[test]
    fn cached_type_resolver_uses_cached_hit_without_inner_call() {
        use std::cell::Cell;
        use std::rc::Rc;
        use throw_trace_core::TypeResolver;

        struct CountingResolver {
            calls: Rc<Cell<usize>>,
        }

        impl TypeResolver for CountingResolver {
            fn is_assignable_to(
                &mut self,
                _file_path: &Path,
                _thrown_type: &str,
                _declared_type: &str,
            ) -> bool {
                self.calls.set(self.calls.get() + 1);
                false
            }

            fn resolve_type(&mut self, _file_path: &Path, _span: Span) -> Option<String> {
                None
            }
        }

        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let file_path = temp_dir.path().join("example.ts");
        fs::write(&file_path, "class ThrownError extends DeclaredError {}")
            .expect("source should be written");
        let source = fs::read_to_string(&file_path).expect("source should be readable");
        let file_hash = CacheStore::content_hash(&source);
        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());
        let scope_fingerprint = "scope".to_string();
        store.update_type_check(
            &file_path,
            &file_hash,
            &scope_fingerprint,
            "ThrownError",
            "DeclaredError",
            true,
        );
        let calls = Rc::new(Cell::new(0));

        let mut resolver = CachedTypeResolver::new(
            &mut store,
            CountingResolver { calls: calls.clone() },
            scope_fingerprint,
        );

        assert!(resolver.is_assignable_to(&file_path, "ThrownError", "DeclaredError"));
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn definition_lookup_round_trips_when_definition_file_unchanged() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let caller_file = temp_dir.path().join("caller.ts");
        let definition_file = temp_dir.path().join("definition.ts");
        fs::write(&caller_file, "foo();").expect("caller should be written");
        fs::write(&definition_file, "function foo() {}").expect("definition should be written");

        let source = fs::read_to_string(&caller_file).expect("caller should be readable");
        let content_hash = CacheStore::content_hash(&source);
        let span = Span { start: 0, end: 3 };
        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        store.update_definition(
            temp_dir.path(),
            &caller_file,
            content_hash.clone(),
            span,
            "foo".to_string(),
            (definition_file.clone(), 1),
        );

        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            Some((definition_file, 1))
        );
    }

    #[test]
    fn definition_lookup_misses_when_definition_file_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let caller_file = temp_dir.path().join("caller.ts");
        let definition_file = temp_dir.path().join("definition.ts");
        fs::write(&caller_file, "foo();").expect("caller should be written");
        fs::write(&definition_file, "function foo() {}").expect("definition should be written");

        let source = fs::read_to_string(&caller_file).expect("caller should be readable");
        let content_hash = CacheStore::content_hash(&source);
        let span = Span { start: 0, end: 3 };
        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        store.update_definition(
            temp_dir.path(),
            &caller_file,
            content_hash.clone(),
            span,
            "foo".to_string(),
            (definition_file.clone(), 1),
        );
        fs::write(&definition_file, "function foo() { return 1; }")
            .expect("definition should be updated");

        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            None
        );
    }

    #[test]
    fn definition_lookup_misses_when_definition_resolution_file_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let caller_file = temp_dir.path().join("caller.ts");
        let definition_dir = temp_dir.path().join("pkg/src");
        fs::create_dir_all(&definition_dir).expect("definition dir should be created");
        let definition_file = definition_dir.join("definition.ts");
        let definition_tsconfig = temp_dir.path().join("pkg/tsconfig.json");
        fs::write(&caller_file, "foo();").expect("caller should be written");
        fs::write(&definition_file, "function foo() {}").expect("definition should be written");
        fs::write(&definition_tsconfig, r#"{"compilerOptions":{}}"#)
            .expect("definition tsconfig should be written");

        let source = fs::read_to_string(&caller_file).expect("caller should be readable");
        let content_hash = CacheStore::content_hash(&source);
        let span = Span { start: 0, end: 3 };
        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        store.update_definition(
            temp_dir.path(),
            &caller_file,
            content_hash.clone(),
            span,
            "foo".to_string(),
            (definition_file.clone(), 1),
        );
        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            Some((definition_file.clone(), 1))
        );

        fs::write(&definition_tsconfig, r#"{"compilerOptions":{"paths":{"@/*":["src/*"]}}}"#)
            .expect("definition tsconfig should be updated");

        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            None
        );
    }

    #[test]
    fn resolution_fingerprint_changes_when_tsconfig_extends_target_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir should be created");
        let tsconfig = temp_dir.path().join("tsconfig.json");
        let base_config = config_dir.join("base.json");
        fs::write(&tsconfig, r#"{"extends":"./config/base.json"}"#)
            .expect("tsconfig should be written");
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("base config should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("base config should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);

        assert_ne!(first, second);
    }

    #[test]
    fn resolution_fingerprint_follows_jsonc_extends() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let tsconfig = temp_dir.path().join("tsconfig.json");
        let base_config = temp_dir.path().join("base.json");
        fs::write(&tsconfig, "{ // comment\n \"extends\": \"./base.json\",\n }")
            .expect("tsconfig should be written");
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("base config should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("base config should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);

        assert_ne!(first, second);
    }

    #[test]
    fn resolution_fingerprint_changes_when_nested_extended_config_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir should be created");
        let tsconfig = temp_dir.path().join("tsconfig.json");
        let base_config = config_dir.join("base.json");
        let shared_config = temp_dir.path().join("shared.json");
        fs::write(&tsconfig, r#"{"extends":"./config/base.json"}"#)
            .expect("tsconfig should be written");
        fs::write(&base_config, r#"{"extends":"../shared.json"}"#)
            .expect("base config should be written");
        fs::write(&shared_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("shared config should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&shared_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("shared config should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);

        assert_ne!(first, second);
    }

    #[test]
    fn resolution_fingerprint_changes_when_extends_array_target_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let tsconfig = temp_dir.path().join("tsconfig.json");
        let base_config = temp_dir.path().join("base.json");
        let env_config = temp_dir.path().join("env.json");
        fs::write(&tsconfig, r#"{"extends":["./base.json","./env.json"]}"#)
            .expect("tsconfig should be written");
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("base config should be written");
        fs::write(&env_config, r#"{"compilerOptions":{"moduleResolution":"bundler"}}"#)
            .expect("env config should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("base config should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        assert_ne!(first, second);

        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("base config should be restored");
        let third = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&env_config, r#"{"compilerOptions":{"moduleResolution":"node16"}}"#)
            .expect("env config should be updated");
        let fourth = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        assert_ne!(third, fourth);
    }

    #[test]
    fn resolution_fingerprint_includes_extensionless_project_reference_without_dot_slash() {
        let workspace_dir = tempfile::tempdir().expect("workspace tempdir should be created");
        let external_dir = tempfile::tempdir().expect("external tempdir should be created");
        let external_src = external_dir.path().join("src");
        let package_dir = external_dir.path().join("packages/a");
        fs::create_dir_all(&external_src).expect("external src should be created");
        fs::create_dir_all(&package_dir).expect("package dir should be created");
        let source_file = external_src.join("a.ts");
        let root_tsconfig = external_dir.path().join("tsconfig.json");
        let package_tsconfig = package_dir.join("tsconfig.json");
        fs::write(&source_file, "export const a = 1;").expect("source should be written");
        fs::write(&root_tsconfig, r#"{"references":[{"path":"packages/a"}]}"#)
            .expect("root tsconfig should be written");
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("package tsconfig should be written");

        let first = resolution_fingerprint_for_files(
            workspace_dir.path(),
            std::slice::from_ref(&source_file),
        );
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("package tsconfig should be updated");
        let second = resolution_fingerprint_for_files(workspace_dir.path(), &[source_file]);

        assert_ne!(first, second);
    }

    #[test]
    fn package_style_extends_adds_marker_when_target_missing() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(
            temp_dir.path().join("tsconfig.json"),
            r#"{"extends":"@tsconfig/node20/tsconfig.json"}"#,
        )
        .expect("tsconfig should be written");

        let entries = resolution_fingerprint_entries_for_files(temp_dir.path(), &[]);

        assert!(entries.iter().any(|(key, value)| {
            key == "tsconfig.json::extends::@tsconfig/node20/tsconfig.json"
                && value == "<package-ref>"
        }));
    }

    #[test]
    fn resolution_fingerprint_changes_when_package_style_extends_target_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let package_dir = temp_dir.path().join("node_modules/@tsconfig/node20");
        fs::create_dir_all(&package_dir).expect("package dir should be created");
        let tsconfig = temp_dir.path().join("tsconfig.json");
        let package_tsconfig = package_dir.join("tsconfig.json");
        fs::write(&tsconfig, r#"{"extends":"@tsconfig/node20/tsconfig.json"}"#)
            .expect("tsconfig should be written");
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"moduleResolution":"bundler"}}"#)
            .expect("package tsconfig should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"moduleResolution":"node16"}}"#)
            .expect("package tsconfig should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);

        assert_ne!(first, second);
    }

    #[test]
    fn resolution_fingerprint_changes_when_package_style_extends_subpath_json_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let package_dir = temp_dir.path().join("node_modules/@acme/tsconfig");
        fs::create_dir_all(&package_dir).expect("package dir should be created");
        let tsconfig = temp_dir.path().join("tsconfig.json");
        let base_config = package_dir.join("base.json");
        fs::write(&tsconfig, r#"{"extends":"@acme/tsconfig/base"}"#)
            .expect("tsconfig should be written");
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("base config should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("base config should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);

        assert_ne!(first, second);
    }

    #[test]
    fn resolution_fingerprint_changes_when_package_tsconfig_field_target_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let package_dir = temp_dir.path().join("node_modules/@acme/tsconfig");
        let config_dir = package_dir.join("configs");
        fs::create_dir_all(&config_dir).expect("config dir should be created");
        let tsconfig = temp_dir.path().join("tsconfig.json");
        let package_json = package_dir.join("package.json");
        let package_config = config_dir.join("node.json");
        fs::write(&tsconfig, r#"{"extends":"@acme/tsconfig"}"#)
            .expect("tsconfig should be written");
        fs::write(&package_json, r#"{"tsconfig":"configs/node.json"}"#)
            .expect("package json should be written");
        fs::write(&package_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("package config should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&package_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("package config should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        assert_ne!(first, second);

        fs::write(&package_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("package config should be restored");
        let third = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&package_json, r#"{"tsconfig":"configs/next.json"}"#)
            .expect("package json should be updated");
        fs::write(config_dir.join("next.json"), r#"{"compilerOptions":{"baseUrl":"next"}}"#)
            .expect("next package config should be written");
        let fourth = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        assert_ne!(third, fourth);

        fs::write(&package_json, r#"{"tsconfig":"configs/node.json"}"#)
            .expect("package json should be restored");
        let fifth = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&package_json, r#"{"name":"@acme/tsconfig","tsconfig":"configs/node.json"}"#)
            .expect("package json content should be updated");
        let sixth = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        assert_ne!(fifth, sixth);
    }

    #[test]
    fn resolution_fingerprint_includes_non_string_reference_marker() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(temp_dir.path().join("tsconfig.json"), r#"{"references":[{"path":123},{}]}"#)
            .expect("tsconfig should be written");

        let entries = resolution_fingerprint_entries_for_files(temp_dir.path(), &[]);

        assert!(entries
            .iter()
            .any(|(key, value)| key == "tsconfig.json::references::<non-string:0>"
                && value == "<invalid>"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "tsconfig.json::references::<missing:1>"
                && value == "<invalid>"));
    }

    #[test]
    fn definition_lookup_misses_when_extended_config_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let caller_file = temp_dir.path().join("caller.ts");
        let definition_dir = temp_dir.path().join("pkg/src");
        let config_dir = temp_dir.path().join("pkg/config");
        fs::create_dir_all(&definition_dir).expect("definition dir should be created");
        fs::create_dir_all(&config_dir).expect("config dir should be created");
        let definition_file = definition_dir.join("definition.ts");
        let definition_tsconfig = temp_dir.path().join("pkg/tsconfig.json");
        let base_config = config_dir.join("base.json");
        fs::write(&caller_file, "foo();").expect("caller should be written");
        fs::write(&definition_file, "function foo() {}").expect("definition should be written");
        fs::write(&definition_tsconfig, r#"{"extends":"./config/base.json"}"#)
            .expect("definition tsconfig should be written");
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("base config should be written");

        let source = fs::read_to_string(&caller_file).expect("caller should be readable");
        let content_hash = CacheStore::content_hash(&source);
        let span = Span { start: 0, end: 3 };
        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        store.update_definition(
            temp_dir.path(),
            &caller_file,
            content_hash.clone(),
            span,
            "foo".to_string(),
            (definition_file.clone(), 1),
        );
        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            Some((definition_file.clone(), 1))
        );

        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("base config should be updated");

        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            None
        );
    }

    #[test]
    fn definition_lookup_misses_when_jsonc_extended_config_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let caller_file = temp_dir.path().join("caller.ts");
        let definition_dir = temp_dir.path().join("pkg/src");
        fs::create_dir_all(&definition_dir).expect("definition dir should be created");
        let definition_file = definition_dir.join("definition.ts");
        let definition_tsconfig = temp_dir.path().join("pkg/tsconfig.json");
        let base_config = temp_dir.path().join("pkg/base.json");
        fs::write(&caller_file, "foo();").expect("caller should be written");
        fs::write(&definition_file, "function foo() {}").expect("definition should be written");
        fs::write(&definition_tsconfig, "{ // comment\n \"extends\": \"./base.json\",\n }")
            .expect("definition tsconfig should be written");
        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("base config should be written");

        let source = fs::read_to_string(&caller_file).expect("caller should be readable");
        let content_hash = CacheStore::content_hash(&source);
        let span = Span { start: 0, end: 3 };
        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        store.update_definition(
            temp_dir.path(),
            &caller_file,
            content_hash.clone(),
            span,
            "foo".to_string(),
            (definition_file.clone(), 1),
        );
        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            Some((definition_file.clone(), 1))
        );

        fs::write(&base_config, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("base config should be updated");

        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            None
        );
    }

    #[test]
    fn resolution_fingerprint_includes_references_tsconfig() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let package_dir = temp_dir.path().join("packages/a");
        fs::create_dir_all(&package_dir).expect("package dir should be created");
        let root_tsconfig = temp_dir.path().join("tsconfig.json");
        let package_tsconfig = package_dir.join("tsconfig.json");
        fs::write(&root_tsconfig, r#"{"references":[{"path":"./packages/a"}]}"#)
            .expect("root tsconfig should be written");
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("package tsconfig should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("package tsconfig should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);

        assert_ne!(first, second);
    }

    #[test]
    fn resolution_fingerprint_follows_jsonc_references() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let package_dir = temp_dir.path().join("packages/a");
        fs::create_dir_all(&package_dir).expect("package dir should be created");
        let root_tsconfig = temp_dir.path().join("tsconfig.json");
        let package_tsconfig = package_dir.join("tsconfig.json");
        fs::write(
            &root_tsconfig,
            "{ // comment\n \"references\": [{ \"path\": \"packages/a\" },],\n }",
        )
        .expect("root tsconfig should be written");
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"baseUrl":"src"}}"#)
            .expect("package tsconfig should be written");

        let first = resolution_fingerprint_for_files(temp_dir.path(), &[]);
        fs::write(&package_tsconfig, r#"{"compilerOptions":{"baseUrl":"lib"}}"#)
            .expect("package tsconfig should be updated");
        let second = resolution_fingerprint_for_files(temp_dir.path(), &[]);

        assert_ne!(first, second);
    }

    #[test]
    fn definition_lookup_misses_for_legacy_entry_without_definition_file_hash() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let caller_file = temp_dir.path().join("caller.ts");
        let definition_file = temp_dir.path().join("definition.ts");
        fs::write(&caller_file, "foo();").expect("caller should be written");
        fs::write(&definition_file, "function foo() {}").expect("definition should be written");

        let source = fs::read_to_string(&caller_file).expect("caller should be readable");
        let content_hash = CacheStore::content_hash(&source);
        let span = Span { start: 0, end: 3 };
        let key = CacheStore::definition_key(&caller_file, &content_hash, span, "foo");
        let mut definitions = serde_json::Map::new();
        definitions.insert(
            key,
            serde_json::json!({
                "caller_file": caller_file,
                "caller_hash": content_hash,
                "callee_span": span,
                "callee_text": "foo",
                "definition_file": definition_file,
                "definition_line": 1,
            }),
        );
        let payload = serde_json::json!({
            "schema_version": CACHE_SCHEMA_VERSION,
            "tool_version": env!("CARGO_PKG_VERSION"),
            "workspace_fingerprint": "workspace",
            "files": {},
            "definitions": definitions,
            "diagnostics": {},
            "type_checks": {},
        });
        fs::create_dir_all(temp_dir.path().join(".throw-trace"))
            .expect("cache dir should be created");
        fs::write(temp_dir.path().join(".throw-trace/cache.json"), payload.to_string())
            .expect("legacy cache should be written");

        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            None
        );
    }

    #[test]
    fn legacy_definition_without_resolution_fingerprint_misses() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let caller_file = temp_dir.path().join("caller.ts");
        let definition_file = temp_dir.path().join("definition.ts");
        fs::write(&caller_file, "foo();").expect("caller should be written");
        fs::write(&definition_file, "function foo() {}").expect("definition should be written");

        let source = fs::read_to_string(&caller_file).expect("caller should be readable");
        let content_hash = CacheStore::content_hash(&source);
        let definition_hash =
            CacheStore::content_hash(&fs::read_to_string(&definition_file).unwrap());
        let span = Span { start: 0, end: 3 };
        let key = CacheStore::definition_key(&caller_file, &content_hash, span, "foo");
        let mut definitions = serde_json::Map::new();
        definitions.insert(
            key,
            serde_json::json!({
                "caller_file": caller_file,
                "caller_hash": content_hash,
                "callee_span": span,
                "callee_text": "foo",
                "definition_file": definition_file,
                "definition_file_hash": definition_hash,
                "definition_line": 1,
            }),
        );
        let payload = serde_json::json!({
            "schema_version": CACHE_SCHEMA_VERSION,
            "tool_version": env!("CARGO_PKG_VERSION"),
            "workspace_fingerprint": "workspace",
            "files": {},
            "definitions": definitions,
            "diagnostics": {},
            "type_checks": {},
        });
        fs::create_dir_all(temp_dir.path().join(".throw-trace"))
            .expect("cache dir should be created");
        fs::write(temp_dir.path().join(".throw-trace/cache.json"), payload.to_string())
            .expect("legacy cache should be written");

        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        assert_eq!(
            store.lookup_definition(temp_dir.path(), &caller_file, &content_hash, span, "foo"),
            None
        );
    }

    #[test]
    fn load_preserves_files_when_workspace_fingerprint_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let mut cache = cache_with_file("old-workspace");
        cache.definitions.insert(
            "definition".to_string(),
            CachedDefinition {
                caller_file: PathBuf::from("src/example.ts"),
                caller_hash: "caller-hash".to_string(),
                callee_span: Span { start: 0, end: 1 },
                callee_text: "callee".to_string(),
                definition_file: PathBuf::from("src/definition.ts"),
                definition_file_hash: "definition-hash".to_string(),
                resolution_fingerprint: "resolution-fingerprint".to_string(),
                definition_line: 1,
            },
        );
        cache.diagnostics.insert(
            "diagnostic".to_string(),
            CachedDiagnostic {
                dependency_fingerprint: "old-dependency".to_string(),
                type_check_dependencies: Vec::new(),
                diagnostic: None,
            },
        );
        cache.type_checks.insert("type-check".to_string(), true);
        write_cache(temp_dir.path(), &cache);

        let store = CacheStore::load(temp_dir.path(), "new-workspace".to_string());

        assert_eq!(store.data.files.len(), 1);
        assert!(store.data.definitions.is_empty());
        assert!(store.data.diagnostics.is_empty());
        assert!(store.data.type_checks.is_empty());
        assert_eq!(store.data.workspace_fingerprint, "new-workspace");
    }

    #[test]
    fn deferred_workspace_validation_preserves_workspace_caches_until_update() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let mut cache = cache_with_file("old-workspace");
        cache.definitions.insert(
            "definition".to_string(),
            CachedDefinition {
                caller_file: PathBuf::from("src/example.ts"),
                caller_hash: "caller-hash".to_string(),
                callee_span: Span { start: 0, end: 1 },
                callee_text: "callee".to_string(),
                definition_file: PathBuf::from("src/definition.ts"),
                definition_file_hash: "definition-hash".to_string(),
                resolution_fingerprint: "resolution-fingerprint".to_string(),
                definition_line: 1,
            },
        );
        cache.diagnostics.insert(
            "diagnostic".to_string(),
            CachedDiagnostic {
                dependency_fingerprint: "old-dependency".to_string(),
                type_check_dependencies: Vec::new(),
                diagnostic: None,
            },
        );
        cache.type_checks.insert("type-check".to_string(), true);
        write_cache(temp_dir.path(), &cache);

        let mut store = CacheStore::load_deferred_workspace_validation(
            temp_dir.path(),
            "initial-workspace".to_string(),
        );

        assert_eq!(store.data.workspace_fingerprint, "old-workspace");
        assert_eq!(store.data.files.len(), 1);
        assert_eq!(store.data.definitions.len(), 1);
        assert_eq!(store.data.diagnostics.len(), 1);
        assert_eq!(store.data.type_checks.len(), 1);

        store.update_workspace_fingerprint("new-workspace".to_string());

        assert_eq!(store.data.files.len(), 1);
        assert!(store.data.definitions.is_empty());
        assert!(store.data.diagnostics.is_empty());
        assert!(store.data.type_checks.is_empty());
        assert_eq!(store.data.workspace_fingerprint, "new-workspace");
    }

    #[test]
    fn load_discards_cache_when_schema_version_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let mut cache = cache_with_file("workspace");
        cache.schema_version = CACHE_SCHEMA_VERSION + 1;
        write_cache(temp_dir.path(), &cache);

        let store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        assert!(store.data.files.is_empty());
        assert_eq!(store.data.schema_version, CACHE_SCHEMA_VERSION);
    }

    #[test]
    fn load_preserves_files_when_tool_version_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let mut cache = cache_with_file("workspace");
        cache.tool_version = "0.0.0-old".to_string();
        cache.type_checks.insert("type-check".to_string(), true);
        write_cache(temp_dir.path(), &cache);

        let store = CacheStore::load(temp_dir.path(), "workspace".to_string());

        assert_eq!(store.data.files.len(), 1);
        assert!(store.data.definitions.is_empty());
        assert!(store.data.diagnostics.is_empty());
        assert!(store.data.type_checks.is_empty());
        assert_eq!(store.data.tool_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn save_replaces_existing_cache_file() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let cache_path = temp_dir.path().join(".throw-trace").join("cache.json");
        fs::create_dir_all(cache_path.parent().expect("cache path should have parent"))
            .expect("cache dir should be created");
        fs::write(&cache_path, "not json").expect("existing cache should be written");

        let mut data = CacheFile::new("workspace".to_string());
        data.type_checks.insert("check".to_string(), true);
        let mut store = CacheStore {
            path: cache_path.clone(),
            workspace_root: canonical_or_self(temp_dir.path()),
            data,
            dirty: true,
            workspace_resolution_fingerprint_cache: None,
        };

        store.save().expect("cache should save over existing file");

        let saved = fs::read_to_string(cache_path).expect("cache should be readable");
        let cache = serde_json::from_str::<CacheFile>(&saved).expect("cache should be valid json");
        assert_eq!(cache.workspace_fingerprint, "workspace");
        assert_eq!(cache.type_checks.get("check"), Some(&true));
        assert!(!store.dirty);
    }

    fn sample_function_id() -> FunctionId {
        FunctionId::new(PathBuf::from("/tmp/a.ts"), "f", Span { start: 0, end: 10 })
    }

    // 診断が依存した型チェックの対象ファイルが変わったら、
    // fingerprint が一致していてもキャッシュを miss させる
    #[test]
    fn diagnostic_lookup_misses_when_type_check_dependency_file_changes() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let dep_file = temp_dir.path().join("errors.ts");
        fs::write(&dep_file, "export class A extends Error {}\n").expect("dep should be written");

        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());
        let func_id = sample_function_id();
        let dep = TypeCheckDependency {
            file: dep_file.clone(),
            file_hash: CacheStore::content_hash("export class A extends Error {}\n"),
            thrown: "A".to_string(),
            declared: "Error".to_string(),
            result: true,
        };
        store.update_diagnostic(&func_id, "fp".to_string(), vec![dep], None);

        assert_eq!(store.lookup_diagnostic(&func_id, "fp"), Some(None));

        fs::write(&dep_file, "export class A {}\n").expect("dep should be updated");
        assert_eq!(
            store.lookup_diagnostic(&func_id, "fp"),
            None,
            "changed dependency file must invalidate the diagnostic entry"
        );
    }

    struct StubResolver(bool);

    impl TypeResolver for StubResolver {
        fn is_assignable_to(&mut self, _: &Path, _: &str, _: &str) -> bool {
            self.0
        }

        fn resolve_type(&mut self, _: &Path, _: Span) -> Option<String> {
            None
        }
    }

    // CachedTypeResolver は参照した型チェックを依存として記録する
    // （キャッシュヒット時も記録されること）
    #[test]
    fn cached_type_resolver_records_type_check_dependencies() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let file = temp_dir.path().join("a.ts");
        let content = "export class Thrown extends Declared {}\n";
        fs::write(&file, content).expect("file should be written");

        let mut store = CacheStore::load(temp_dir.path(), "workspace".to_string());
        let mut resolver =
            CachedTypeResolver::new(&mut store, StubResolver(true), "scope".to_string());

        assert!(resolver.is_assignable_to(&file, "Thrown", "Declared"));
        let recorded = resolver.take_recorded();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].file_hash, CacheStore::content_hash(content));
        assert_eq!(recorded[0].thrown, "Thrown");
        assert_eq!(recorded[0].declared, "Declared");
        assert!(recorded[0].result);
        assert!(resolver.take_recorded().is_empty(), "take_recorded should drain");

        // 2回目はキャッシュヒット経路だが、依存としては同様に記録される
        assert!(resolver.is_assignable_to(&file, "Thrown", "Declared"));
        assert_eq!(resolver.take_recorded().len(), 1);
    }
}

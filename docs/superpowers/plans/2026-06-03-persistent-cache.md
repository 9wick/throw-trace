# Persistent Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a conservative persistent cache at `.throw-trace/cache.json` so large codebases can skip unchanged file extraction, definition resolution, and function diagnostics work.

**Architecture:** Keep cache ownership in the CLI crate near `Analyzer`, because `check` and `fix` both use that shared pipeline. Add a `cache.rs` module with load/save, stable hashing, file extraction entries, definition entries, diagnostics entries, and a cached `TypeResolver` wrapper. Integrate it incrementally: first file extraction cache, then definition cache, then diagnostics cache.

**Tech Stack:** Rust 1.85, serde/serde_json, sha2, hex, existing `throw_trace_core` serializable types, existing `Analyzer`, existing tsserver resolver.

---

## File Structure

- Create `crates/throw-trace/src/cache.rs`
  - Owns `.throw-trace/cache.json` schema, load/save, stable hashing, cache lookup/update APIs, and `CachedTypeResolver`.
- Modify `crates/throw-trace/src/main.rs`
  - Add `mod cache;`.
- Modify `crates/throw-trace/src/analyzer.rs`
  - Add `CacheStore` field.
  - Use file extraction cache in `analyze_file`.
  - Use definition cache in `collect_definition_targets`.
  - Save cache at the end of `analyze_files`.
  - Use diagnostics cache in `generate_diagnostics`.
  - Use cached type resolver wrapper in diagnostics generation.
- Modify `Cargo.toml`
  - Add workspace dependencies `sha2` and `hex`.
- Modify `crates/throw-trace/Cargo.toml`
  - Add `sha2.workspace = true` and `hex.workspace = true`.
- Add `crates/throw-trace/tests/cache_test.rs`
  - CLI-level tests for cache file creation, cache reuse, invalidation, schema mismatch, corrupt JSON fallback, and `fix` using the shared analyzer cache path.

---

### Task 1: Cache Module Skeleton And Stable Hashing

**Files:**
- Create: `crates/throw-trace/src/cache.rs`
- Modify: `crates/throw-trace/src/main.rs`
- Modify: `Cargo.toml`
- Modify: `crates/throw-trace/Cargo.toml`
- Test: `crates/throw-trace/src/cache.rs`

- [ ] **Step 1: Add dependencies**

Edit workspace dependencies in `Cargo.toml`:

```toml
# Utility
compact_str = { version = "=0.8.1", features = ["serde"] }
smallvec = { version = "=1.13.2", features = ["serde", "union"] }
sha2 = "=0.10.8"
hex = "=0.4.3"
```

Edit `crates/throw-trace/Cargo.toml` dependencies:

```toml
[dependencies]
throw-trace-core.workspace = true
throw-trace-ts.workspace = true
clap.workspace = true
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
ignore.workspace = true
globset.workspace = true
sha2.workspace = true
hex.workspace = true
```

- [ ] **Step 2: Create `cache.rs` with schema and hashing utilities**

Create `crates/throw-trace/src/cache.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use throw_trace_core::{
    Diagnostic, FunctionId, FunctionSignature, MethodSignature, Span, TypeRelation,
};

pub const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheFile {
    pub schema_version: u32,
    pub tool_version: String,
    pub workspace_fingerprint: String,
    pub files: BTreeMap<PathBuf, CachedFile>,
    pub definitions: BTreeMap<String, CachedDefinition>,
    pub diagnostics: BTreeMap<String, CachedDiagnostic>,
    pub type_checks: BTreeMap<String, bool>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDefinition {
    pub caller_file: PathBuf,
    pub caller_hash: String,
    pub callee_span: Span,
    pub callee_text: String,
    pub definition_file: PathBuf,
    pub definition_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDiagnostic {
    pub dependency_fingerprint: String,
    pub diagnostic: Option<Diagnostic>,
}

#[derive(Debug)]
pub struct CacheStore {
    path: PathBuf,
    data: CacheFile,
    dirty: bool,
}

impl CacheStore {
    pub fn load(workspace_root: &Path, workspace_fingerprint: String) -> Self {
        let path = workspace_root.join(".throw-trace").join("cache.json");
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<CacheFile>(&raw).ok())
            .filter(|cache| cache.schema_version == CACHE_SCHEMA_VERSION)
            .unwrap_or_else(|| CacheFile {
                schema_version: CACHE_SCHEMA_VERSION,
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                workspace_fingerprint: workspace_fingerprint.clone(),
                files: BTreeMap::new(),
                definitions: BTreeMap::new(),
                diagnostics: BTreeMap::new(),
                type_checks: BTreeMap::new(),
            });

        Self { path, data: CacheFile { workspace_fingerprint, ..data }, dirty: false }
    }

    pub fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = self.path.with_extension("json.tmp");
        let payload = serde_json::to_string_pretty(&self.data)?;
        fs::write(&tmp_path, payload)?;
        fs::rename(tmp_path, &self.path)?;
        Ok(())
    }

    pub fn content_hash(source: &str) -> String {
        hash_bytes(source.as_bytes())
    }

    pub fn stable_json_hash<T: Serialize>(value: &T) -> String {
        let bytes = serde_json::to_vec(value).unwrap_or_default();
        hash_bytes(&bytes)
    }
}

pub fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

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

    #[test]
    fn content_hash_is_stable() {
        assert_eq!(CacheStore::content_hash("abc"), CacheStore::content_hash("abc"));
        assert_ne!(CacheStore::content_hash("abc"), CacheStore::content_hash("abcd"));
    }

    #[test]
    fn stable_json_hash_uses_btree_order() {
        let mut value = BTreeMap::new();
        value.insert("b", "2");
        value.insert("a", "1");
        assert_eq!(CacheStore::stable_json_hash(&value), CacheStore::stable_json_hash(&value));
    }
}
```

- [ ] **Step 3: Register module**

Edit `crates/throw-trace/src/main.rs`:

```rust
mod analyzer;
mod cache;
mod fixer;
mod loader;
mod reporter;
```

- [ ] **Step 4: Run cache unit tests**

Run:

```bash
cargo test -p throw-trace cache::tests
```

Expected: tests compile and pass.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/throw-trace/Cargo.toml crates/throw-trace/src/main.rs crates/throw-trace/src/cache.rs Cargo.lock
git commit -m "feat: add persistent cache store skeleton"
```

---

### Task 2: File Extraction Cache

**Files:**
- Modify: `crates/throw-trace/src/cache.rs`
- Modify: `crates/throw-trace/src/analyzer.rs`
- Test: `crates/throw-trace/tests/cache_test.rs`

- [ ] **Step 1: Add file extraction lookup/update APIs**

Add these methods to `impl CacheStore` in `cache.rs`:

```rust
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
```

- [ ] **Step 2: Add `ExtractionResult` conversion helper**

Because `throw_trace_ts::ExtractionResult` is not serializable itself, convert through `CachedExtraction`.

Add to `cache.rs`:

```rust
impl CachedExtraction {
    pub fn from_parts(
        signatures: Vec<FunctionSignature>,
        method_signatures: Vec<MethodSignature>,
        type_relations: Vec<TypeRelation>,
    ) -> Self {
        Self { signatures, method_signatures, type_relations }
    }
}
```

- [ ] **Step 3: Add cache field to `Analyzer`**

Modify imports in `analyzer.rs`:

```rust
use crate::cache::{CacheStore, CachedExtraction};
```

Add field:

```rust
cache: CacheStore,
```

Update `Analyzer::with_config`:

```rust
let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
let workspace_fingerprint = CacheStore::stable_json_hash(&(
    env!("CARGO_PKG_VERSION"),
    config.max_depth,
    config.max_files,
    config.cross_file,
));

Self {
    signatures: HashMap::new(),
    method_signatures: Vec::new(),
    type_relations: Vec::new(),
    graph: CallGraph::new(),
    entry_files: HashSet::new(),
    analyzed_files: HashSet::new(),
    config,
    resolved_calls: HashMap::new(),
    cache: CacheStore::load(&workspace_root, workspace_fingerprint),
}
```

- [ ] **Step 4: Use cache in `analyze_file`**

Replace the extraction block in `Analyzer::analyze_file`:

```rust
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
```

- [ ] **Step 5: Save cache after analysis**

At the end of `Analyzer::analyze_files`, before `Ok(())`:

```rust
self.build_call_graph();
let _ = self.cache.save();
Ok(())
```

- [ ] **Step 6: Add CLI cache creation test**

Create `crates/throw-trace/tests/cache_test.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap()
}

#[test]
fn check_creates_persistent_cache_file() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));

    assert!(temp.path().join(".throw-trace/cache.json").exists());
}
```

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test -p throw-trace check_creates_persistent_cache_file
cargo test -p throw-trace cache::tests
```

Expected: all selected tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/throw-trace/src/cache.rs crates/throw-trace/src/analyzer.rs crates/throw-trace/tests/cache_test.rs
git commit -m "feat: cache file extraction results"
```

---

### Task 3: Definition Cache

**Files:**
- Modify: `crates/throw-trace/src/cache.rs`
- Modify: `crates/throw-trace/src/analyzer.rs`
- Test: `crates/throw-trace/src/cache.rs`

- [ ] **Step 1: Add definition cache APIs**

Add to `impl CacheStore` in `cache.rs`:

```rust
pub fn definition_key(file_path: &Path, content_hash: &str, callee_span: Span, callee_text: &str) -> String {
    let mut parts = BTreeMap::new();
    parts.insert("caller_file", canonical_or_self(file_path).display().to_string());
    parts.insert("caller_hash", content_hash.to_string());
    parts.insert("span_start", callee_span.start.to_string());
    parts.insert("span_end", callee_span.end.to_string());
    parts.insert("callee_text", callee_text.to_string());
    Self::stable_json_hash(&parts)
}

pub fn lookup_definition(
    &self,
    file_path: &Path,
    content_hash: &str,
    callee_span: Span,
    callee_text: &str,
) -> Option<(PathBuf, u32)> {
    let key = Self::definition_key(file_path, content_hash, callee_span, callee_text);
    let entry = self.data.definitions.get(&key)?;
    if entry.caller_hash == content_hash
        && entry.callee_span == callee_span
        && entry.callee_text == callee_text
    {
        Some((entry.definition_file.clone(), entry.definition_line))
    } else {
        None
    }
}

pub fn update_definition(
    &mut self,
    file_path: &Path,
    content_hash: String,
    callee_span: Span,
    callee_text: String,
    definition_file: PathBuf,
    definition_line: u32,
) {
    let key = Self::definition_key(file_path, &content_hash, callee_span, &callee_text);
    self.data.definitions.insert(
        key,
        CachedDefinition {
            caller_file: canonical_or_self(file_path),
            caller_hash: content_hash,
            callee_span,
            callee_text,
            definition_file,
            definition_line,
        },
    );
    self.dirty = true;
}
```

- [ ] **Step 2: Add definition key unit test**

Add to `cache.rs` tests:

```rust
#[test]
fn definition_key_changes_when_callee_text_changes() {
    let path = Path::new("/tmp/a.ts");
    let span = Span { start: 10, end: 13 };
    let first = CacheStore::definition_key(path, "hash", span, "foo");
    let second = CacheStore::definition_key(path, "hash", span, "bar");
    assert_ne!(first, second);
}
```

- [ ] **Step 3: Use definition cache in `collect_definition_targets`**

Inside the loop in `Analyzer::collect_definition_targets`, after reading `source`:

```rust
let content_hash = CacheStore::content_hash(&source);
let callee_text = source
    .get(callee_span.start as usize..callee_span.end as usize)
    .unwrap_or_default()
    .to_string();

if let Some((def_file, def_line)) =
    self.cache.lookup_definition(&file_path, &content_hash, callee_span, &callee_text)
{
    self.resolved_calls.insert(
        (file_path.clone(), callee_span),
        (def_file.clone(), def_line),
    );
    if !self.analyzed_files.contains(&def_file) {
        targets.insert(def_file);
    }
    continue;
}
```

After a successful tsserver definition lookup:

```rust
self.cache.update_definition(
    &file_path,
    content_hash,
    callee_span,
    callee_text,
    canonical.clone(),
    def.start.line,
);
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p throw-trace cache::tests
cargo test -p throw-trace check_cross_file_missing_throws_detected -- --ignored
```

Expected:
- cache unit tests pass
- ignored cross-file test passes when tsserver is installed
- if tsserver is not installed locally, record that the ignored test was not runnable and continue after running the non-ignored suite in Task 6

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace/src/cache.rs crates/throw-trace/src/analyzer.rs
git commit -m "feat: cache tsserver definition lookups"
```

---

### Task 4: Cached Type Resolver

**Files:**
- Modify: `crates/throw-trace/src/cache.rs`
- Modify: `crates/throw-trace/src/analyzer.rs`
- Test: `crates/throw-trace/src/cache.rs`

- [ ] **Step 1: Add type check cache APIs**

Add imports in `cache.rs`:

```rust
use throw_trace_core::TypeResolver;
```

Add to `impl CacheStore`:

```rust
pub fn type_check_key(file_path: &Path, thrown_type: &str, declared_type: &str) -> String {
    let mut parts = BTreeMap::new();
    parts.insert("file", canonical_or_self(file_path).display().to_string());
    parts.insert("thrown", thrown_type.to_string());
    parts.insert("declared", declared_type.to_string());
    Self::stable_json_hash(&parts)
}

pub fn lookup_type_check(
    &self,
    file_path: &Path,
    thrown_type: &str,
    declared_type: &str,
) -> Option<bool> {
    let key = Self::type_check_key(file_path, thrown_type, declared_type);
    self.data.type_checks.get(&key).copied()
}

pub fn update_type_check(
    &mut self,
    file_path: &Path,
    thrown_type: &str,
    declared_type: &str,
    result: bool,
) {
    let key = Self::type_check_key(file_path, thrown_type, declared_type);
    self.data.type_checks.insert(key, result);
    self.dirty = true;
}
```

- [ ] **Step 2: Add cached resolver wrapper**

Add to `cache.rs`:

```rust
pub struct CachedTypeResolver<'a, R> {
    cache: &'a mut CacheStore,
    inner: R,
}

impl<'a, R> CachedTypeResolver<'a, R> {
    pub fn new(cache: &'a mut CacheStore, inner: R) -> Self {
        Self { cache, inner }
    }
}

impl<R: TypeResolver> TypeResolver for CachedTypeResolver<'_, R> {
    fn is_assignable_to(
        &mut self,
        file_path: &Path,
        thrown_type: &str,
        declared_type: &str,
    ) -> bool {
        if let Some(result) = self.cache.lookup_type_check(file_path, thrown_type, declared_type) {
            return result;
        }

        let result = self.inner.is_assignable_to(file_path, thrown_type, declared_type);
        self.cache.update_type_check(file_path, thrown_type, declared_type, result);
        result
    }

    fn resolve_type(&mut self, file_path: &Path, span: Span) -> Option<String> {
        self.inner.resolve_type(file_path, span)
    }
}
```

- [ ] **Step 3: Use cached resolver in diagnostics and LSP violation generation**

Modify imports in `analyzer.rs`:

```rust
use crate::cache::{CacheStore, CachedExtraction, CachedTypeResolver};
```

Change resolver construction in `generate_diagnostics`:

```rust
let all_diagnostics = if let Ok(resolver) = TsServerTypeResolver::new() {
    let mut resolver = CachedTypeResolver::new(&mut self.cache, resolver);
    generate_diagnostics_with_resolver(&self.signatures, &self.graph, &mut resolver)
} else {
    eprintln!("warning: tsserver not available, falling back to string comparison");
    generate_diagnostics_with_resolver(
        &self.signatures,
        &self.graph,
        &mut throw_trace_core::NoOpTypeResolver,
    )
};
let _ = self.cache.save();
```

Because this mutates cache, change signatures:

```rust
pub fn generate_diagnostics(&mut self) -> Vec<Diagnostic>
pub fn generate_lsp_violations(&mut self) -> Vec<LspViolation>
```

Change resolver construction in `generate_lsp_violations`:

```rust
let all_violations = if let Ok(resolver) = TsServerTypeResolver::new() {
    let mut resolver = CachedTypeResolver::new(&mut self.cache, resolver);
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
```

Update call sites in `main.rs` already use `let mut analyzer`, so no additional call-site shape change is required.

- [ ] **Step 4: Add type check key unit test**

Add to `cache.rs` tests:

```rust
#[test]
fn type_check_key_includes_declared_type() {
    let path = Path::new("/tmp/a.ts");
    let first = CacheStore::type_check_key(path, "ChildError", "BaseError");
    let second = CacheStore::type_check_key(path, "ChildError", "OtherError");
    assert_ne!(first, second);
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p throw-trace cache::tests
cargo test -p throw-trace check_simple_throw_reports_missing
```

Expected: tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/throw-trace/src/cache.rs crates/throw-trace/src/analyzer.rs crates/throw-trace/src/main.rs
git commit -m "feat: persist type compatibility checks"
```

---

### Task 5: Function Diagnostics Cache

**Files:**
- Modify: `crates/throw-trace/src/cache.rs`
- Modify: `crates/throw-trace/src/analyzer.rs`
- Modify: `crates/throw-trace-core/src/diagnostic.rs`
- Test: `crates/throw-trace/tests/cache_test.rs`

- [ ] **Step 1: Expose per-function missing declaration helper**

In `crates/throw-trace-core/src/diagnostic.rs`, make `find_missing_declarations` public:

```rust
pub fn find_missing_declarations<R: TypeResolver>(
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
                p.origin.location,
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
```

Update `crates/throw-trace-core/src/lib.rs`:

```rust
pub use diagnostic::{
    find_missing_declarations, generate_diagnostics_with_resolver, generate_lsp_violations,
};
```

- [ ] **Step 2: Add diagnostics cache APIs**

Add to `impl CacheStore`:

```rust
pub fn lookup_diagnostic(
    &self,
    function_id: &FunctionId,
    dependency_fingerprint: &str,
) -> Option<Option<Diagnostic>> {
    let key = function_cache_key(function_id);
    let entry = self.data.diagnostics.get(&key)?;
    if entry.dependency_fingerprint == dependency_fingerprint {
        Some(entry.diagnostic.clone())
    } else {
        None
    }
}

pub fn update_diagnostic(
    &mut self,
    function_id: &FunctionId,
    dependency_fingerprint: String,
    diagnostic: Option<Diagnostic>,
) {
    let key = function_cache_key(function_id);
    self.data.diagnostics.insert(
        key,
        CachedDiagnostic { dependency_fingerprint, diagnostic },
    );
    self.dirty = true;
}
```

- [ ] **Step 3: Add dependency fingerprint helper**

Add this method to `Analyzer`:

```rust
fn dependency_fingerprint(&self, func_id: &FunctionId) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(sig) = self.signatures.get(func_id) {
        parts.push(CacheStore::stable_json_hash(sig));
    }

    let mut callees = self.graph.get_transitive_callees(func_id);
    callees.sort_by(|a, b| {
        (
            a.file_path.display().to_string(),
            a.name.to_string(),
            a.span.start,
            a.span.end,
        )
            .cmp(&(
                b.file_path.display().to_string(),
                b.name.to_string(),
                b.span.start,
                b.span.end,
            ))
    });

    for callee in callees {
        if let Some(sig) = self.signatures.get(&callee) {
            parts.push(CacheStore::stable_json_hash(sig));
        }
        for location in self.graph.get_call_site_locations(func_id, &callee) {
            parts.push(CacheStore::stable_json_hash(location));
        }
    }

    let mut methods = self.method_signatures.clone();
    methods.sort_by(|a, b| {
        (
            a.type_id.name.to_string(),
            a.method_name.to_string(),
            a.method_span.start,
            a.method_span.end,
        )
            .cmp(&(
                b.type_id.name.to_string(),
                b.method_name.to_string(),
                b.method_span.start,
                b.method_span.end,
            ))
    });

    let mut relations = self.type_relations.clone();
    relations.sort_by(|a, b| {
        (
            a.child.name.to_string(),
            a.parent.name.to_string(),
            a.child.span.start,
            a.parent.span.start,
        )
            .cmp(&(
                b.child.name.to_string(),
                b.parent.name.to_string(),
                b.child.span.start,
                b.parent.span.start,
            ))
    });

    parts.push(CacheStore::stable_json_hash(&methods));
    parts.push(CacheStore::stable_json_hash(&relations));
    parts.push(self.cache.workspace_fingerprint().to_string());
    CacheStore::stable_json_hash(&parts)
}
```

Add to `CacheStore`:

```rust
pub fn workspace_fingerprint(&self) -> &str {
    &self.data.workspace_fingerprint
}
```

- [ ] **Step 4: Generate diagnostics per function with cache**

Modify imports in `analyzer.rs`:

```rust
use throw_trace_core::{
    compute_propagated_throws, find_missing_declarations, generate_lsp_violations, CallGraph,
    Diagnostic, FunctionId, FunctionSignature, LspViolation, MethodSignature, Span, TypeRelation,
};
```

Replace `generate_diagnostics` with a cache-aware implementation:

```rust
pub fn generate_diagnostics(&mut self) -> Vec<Diagnostic> {
    let function_ids: Vec<FunctionId> = self.signatures.keys().cloned().collect();
    let fingerprints: HashMap<FunctionId, String> = function_ids
        .iter()
        .map(|func_id| (func_id.clone(), self.dependency_fingerprint(func_id)))
        .collect();

    let diagnostics = if let Ok(resolver) = TsServerTypeResolver::new() {
        let mut resolver = CachedTypeResolver::new(&mut self.cache, resolver);
        let mut diagnostics = Vec::new();

        for func_id in function_ids {
            let Some(sig) = self.signatures.get(&func_id) else {
                continue;
            };
            let Some(fingerprint) = fingerprints.get(&func_id).cloned() else {
                continue;
            };

            if let Some(cached) = resolver.cache_mut().lookup_diagnostic(&func_id, &fingerprint) {
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
            resolver.cache_mut().update_diagnostic(&func_id, fingerprint, diagnostic.clone());
            if let Some(diagnostic) = diagnostic {
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    } else {
        eprintln!("warning: tsserver not available, falling back to string comparison");
        let mut resolver =
            CachedTypeResolver::new(&mut self.cache, throw_trace_core::NoOpTypeResolver);
        let mut diagnostics = Vec::new();

        for func_id in function_ids {
            let Some(sig) = self.signatures.get(&func_id) else {
                continue;
            };
            let Some(fingerprint) = fingerprints.get(&func_id).cloned() else {
                continue;
            };

            if let Some(cached) = resolver.cache_mut().lookup_diagnostic(&func_id, &fingerprint) {
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
            resolver.cache_mut().update_diagnostic(&func_id, fingerprint, diagnostic.clone());
            if let Some(diagnostic) = diagnostic {
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    };

    let _ = self.cache.save();

    diagnostics
        .into_iter()
        .filter(|d| self.entry_files.contains(&d.function.file_path))
        .collect()
}
```

Add to `CachedTypeResolver`:

```rust
pub fn cache_mut(&mut self) -> &mut CacheStore {
    self.cache
}
```

- [ ] **Step 5: Add invalidation test for changed source**

Add to `crates/throw-trace/tests/cache_test.rs`:

```rust
#[test]
fn changing_source_updates_cache_file() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure();

    let cache_path = temp.path().join(".throw-trace/cache.json");
    let first = fs::read_to_string(&cache_path).unwrap();

    fs::write(
        &source,
        r#"
/**
 * @throws {Error} documented
 */
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));

    let second = fs::read_to_string(&cache_path).unwrap();
    assert_ne!(first, second);
}
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p throw-trace changing_source_updates_cache_file
cargo test -p throw-trace check_simple_throw_reports_missing
```

Expected: tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/throw-trace-core/src/diagnostic.rs crates/throw-trace-core/src/lib.rs crates/throw-trace/src/cache.rs crates/throw-trace/src/analyzer.rs crates/throw-trace/tests/cache_test.rs
git commit -m "feat: cache function diagnostics"
```

---

### Task 6: Fallback, Schema Mismatch, And Shared Check/Fix Coverage

**Files:**
- Modify: `crates/throw-trace/tests/cache_test.rs`
- Modify: `crates/throw-trace/src/cache.rs`

- [ ] **Step 1: Add schema mismatch fallback test**

Add to `cache_test.rs`:

```rust
#[test]
fn schema_mismatch_cache_is_ignored() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join(".throw-trace");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(
        cache_dir.join("cache.json"),
        r#"{"schema_version":999,"tool_version":"old","workspace_fingerprint":"x","files":{},"definitions":{},"diagnostics":{},"type_checks":{}}"#,
    )
    .unwrap();

    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));
}
```

- [ ] **Step 2: Add corrupt JSON fallback test**

Add to `cache_test.rs`:

```rust
#[test]
fn corrupt_cache_json_is_ignored() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join(".throw-trace");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("cache.json"), "{not json").unwrap();

    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));
}
```

- [ ] **Step 3: Add fix shared cache path test**

Add to `cache_test.rs`:

```rust
#[test]
fn fix_uses_shared_analyzer_cache_path() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["fix", source.to_str().unwrap()])
        .assert()
        .success();

    assert!(temp.path().join(".throw-trace/cache.json").exists());
    let fixed = fs::read_to_string(&source).unwrap();
    assert!(fixed.contains("@throws {Error}"));
}
```

- [ ] **Step 4: Ensure save failures do not fail analysis**

In `cache.rs`, keep all existing caller sites as:

```rust
let _ = self.cache.save();
```

Do not propagate save errors from `Analyzer`.

- [ ] **Step 5: Run cache tests**

Run:

```bash
cargo test -p throw-trace --test cache_test
```

Expected: all cache integration tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/throw-trace/tests/cache_test.rs crates/throw-trace/src/cache.rs
git commit -m "test: cover persistent cache fallback behavior"
```

---

### Task 7: Full Verification And Cleanup

**Files:**
- Modify only files needed by compiler, formatter, or clippy feedback from this task.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt --all
```

Expected: command exits successfully.

- [ ] **Step 2: Run full non-ignored test suite**

Run:

```bash
cargo test --workspace
```

Expected: all non-ignored tests pass.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: no warnings.

- [ ] **Step 4: Inspect generated local cache state**

Run:

```bash
git status --short
```

Expected: `.throw-trace/cache.json` is not staged. If it appears as an untracked file from local test runs, remove only that generated file with:

```bash
rm -f .throw-trace/cache.json
```

- [ ] **Step 5: Commit verification cleanup**

If formatting or clippy changed tracked files:

```bash
git add <changed tracked files>
git commit -m "chore: finalize persistent cache implementation"
```

If no tracked files changed, do not create an empty commit.

---

## Self-Review Notes

- Spec coverage:
  - `.throw-trace/cache.json` load/save: Task 1
  - file extraction cache: Task 2
  - definition cache: Task 3
  - type check cache: Task 4
  - function diagnostics cache: Task 5
  - schema mismatch and corrupt JSON fallback: Task 6
  - shared `check`/`fix` analyzer path: Task 6
  - full verification: Task 7
- Scope check: this plan implements the persistent cache only. It does not add cache location options, external throw databases, or tsserver process persistence.
- Type consistency: cache structs use existing serializable core types; diagnostics cache uses `Diagnostic`; type resolver wrapper implements the existing `TypeResolver` trait.

# throw-trace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TypeScript向け`@throws` TSDoc静的解析ツール。関数が投げる可能性のある例外を追跡し、`@throws`宣言の漏れを検出する。

**Architecture:** 2パス解析。Pass1で全ファイルをパースし関数シグネチャ+@throwsをインデックス化。Pass2でcall graph構築→伝播計算→宣言との突合。monorepo構造（core/ts/cli）で将来の多言語対応を見据える。

**Tech Stack:** Rust, Oxc (parser), petgraph (call graph), clap (CLI), serde_json (output)

---

## File Structure

```
throw-trace/
├── Cargo.toml                          # workspace root
├── rust-toolchain.toml
├── rustfmt.toml
├── .gitignore
├── crates/
│   ├── throw-trace-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # pub exports
│   │       ├── types.rs                # FunctionId, ErrorType, ThrowSite, etc.
│   │       ├── call_graph.rs           # petgraph wrapper, 伝播計算
│   │       ├── propagation.rs          # throw伝播ロジック
│   │       └── diagnostic.rs           # Diagnostic, レポート生成
│   ├── throw-trace-ts/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # pub parse_file()
│   │       ├── parser.rs               # oxcラッパー
│   │       ├── extract.rs              # 関数定義抽出
│   │       ├── jsdoc.rs                # @throws抽出
│   │       ├── throw_analyzer.rs       # throw文解析、フロー解析
│   │       └── try_catch.rs            # try-catch範囲解析
│   └── throw-trace/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                 # エントリーポイント
│           ├── args.rs                 # clap定義
│           ├── loader.rs               # ファイル走査、tsconfig解析
│           ├── analyzer.rs             # 2パス解析オーケストレーション
│           └── reporter.rs             # text/json出力
└── tests/
    └── fixtures/                       # テスト用TSファイル
        ├── simple_throw.ts
        ├── propagation.ts
        ├── try_catch.ts
        └── ...
```

---

## Task 1: Workspace Setup

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`
- Create: `.gitignore`

- [ ] **Step 1: Create workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/throw-trace-core",
    "crates/throw-trace-ts",
    "crates/throw-trace",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
rust-version = "1.85"
authors = ["KoheiKido <kido@9wick.com>"]
repository = "https://github.com/9wick/throw-trace"
homepage = "https://github.com/9wick/throw-trace"

[workspace.dependencies]
# Internal crates
throw-trace-core = { version = "0.1.0", path = "crates/throw-trace-core" }
throw-trace-ts = { version = "0.1.0", path = "crates/throw-trace-ts" }

# Parser
oxc_parser = "=0.73.0"
oxc_ast = "=0.73.0"
oxc_allocator = "=0.73.0"
oxc_span = "=0.73.0"

# Call graph
petgraph = "=0.6.5"

# CLI
clap = { version = "=4.5.21", features = ["derive"] }

# Serialization
serde = { version = "=1.0.215", features = ["derive"] }
serde_json = "=1.0.133"

# Error handling
thiserror = "=2.0.6"
anyhow = "=1.0.94"

# File walking
ignore = "=0.4.23"
globset = "=0.4.15"

# Utility
compact_str = { version = "=0.8.1", features = ["serde"] }
smallvec = { version = "=1.13.2", features = ["serde", "union"] }

# Dev
assert_cmd = "=2.0.16"
predicates = "=3.1.3"
tempfile = "=3.14.0"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
must_use_candidate = "allow"
```

- [ ] **Step 2: Create rust-toolchain.toml**

```toml
[toolchain]
channel = "1.85"
```

- [ ] **Step 3: Create rustfmt.toml**

```toml
edition = "2021"
max_width = 100
use_small_heuristics = "Max"
```

- [ ] **Step 4: Create .gitignore**

```
/target
Cargo.lock
*.swp
*.swo
.DS_Store
```

- [ ] **Step 5: Verify workspace structure**

Run: `ls -la`
Expected: Cargo.toml, rust-toolchain.toml, rustfmt.toml, .gitignore が存在

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml rustfmt.toml .gitignore
git commit -m "$(cat <<'EOF'
chore: initialize Rust workspace for throw-trace
EOF
)"
```

---

## Task 2: throw-trace-core Crate Setup

**Files:**
- Create: `crates/throw-trace-core/Cargo.toml`
- Create: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Create directory**

Run: `mkdir -p crates/throw-trace-core/src`

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "throw-trace-core"
description = "Core engine for throw-trace: types, call graph, propagation analysis."
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
keywords = ["lint", "throws", "static-analysis", "typescript"]
categories = ["development-tools"]

[dependencies]
compact_str.workspace = true
smallvec.workspace = true
thiserror.workspace = true
petgraph.workspace = true
serde.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: Create minimal lib.rs**

```rust
//! Core engine for throw-trace: types, call graph, propagation analysis.

pub fn hello() -> &'static str {
    "throw-trace-core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_returns_crate_name() {
        assert_eq!(hello(), "throw-trace-core");
    }
}
```

- [ ] **Step 4: Verify crate compiles**

Run: `cargo build -p throw-trace-core`
Expected: Compiling throw-trace-core ... Finished

- [ ] **Step 5: Run tests**

Run: `cargo test -p throw-trace-core`
Expected: test tests::hello_returns_crate_name ... ok

- [ ] **Step 6: Commit**

```bash
git add crates/throw-trace-core/
git commit -m "$(cat <<'EOF'
feat(core): add throw-trace-core crate skeleton
EOF
)"
```

---

## Task 3: throw-trace-ts Crate Setup

**Files:**
- Create: `crates/throw-trace-ts/Cargo.toml`
- Create: `crates/throw-trace-ts/src/lib.rs`

- [ ] **Step 1: Create directory**

Run: `mkdir -p crates/throw-trace-ts/src`

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "throw-trace-ts"
description = "TypeScript adapter for throw-trace (oxc-based)."
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
keywords = ["typescript", "javascript", "lint", "ast", "oxc"]
categories = ["development-tools", "parser-implementations"]

[dependencies]
throw-trace-core.workspace = true
oxc_parser.workspace = true
oxc_ast.workspace = true
oxc_allocator.workspace = true
oxc_span.workspace = true
compact_str.workspace = true
thiserror.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: Create minimal lib.rs**

```rust
//! TypeScript adapter for throw-trace (oxc-based).

pub fn hello() -> &'static str {
    "throw-trace-ts"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_returns_crate_name() {
        assert_eq!(hello(), "throw-trace-ts");
    }
}
```

- [ ] **Step 4: Verify crate compiles**

Run: `cargo build -p throw-trace-ts`
Expected: Compiling throw-trace-ts ... Finished

- [ ] **Step 5: Run tests**

Run: `cargo test -p throw-trace-ts`
Expected: test tests::hello_returns_crate_name ... ok

- [ ] **Step 6: Commit**

```bash
git add crates/throw-trace-ts/
git commit -m "$(cat <<'EOF'
feat(ts): add throw-trace-ts crate skeleton
EOF
)"
```

---

## Task 4: throw-trace CLI Crate Setup

**Files:**
- Create: `crates/throw-trace/Cargo.toml`
- Create: `crates/throw-trace/src/main.rs`

- [ ] **Step 1: Create directory**

Run: `mkdir -p crates/throw-trace/src`

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "throw-trace"
description = "Static analysis tool for @throws TSDoc declarations."
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
keywords = ["lint", "throws", "tsdoc", "cli", "typescript"]
categories = ["command-line-utilities", "development-tools"]

[[bin]]
name = "throw-trace"
path = "src/main.rs"

[dependencies]
throw-trace-core.workspace = true
throw-trace-ts.workspace = true
clap.workspace = true
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
ignore.workspace = true
globset.workspace = true

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
tempfile.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: Create minimal main.rs**

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "throw-trace")]
#[command(about = "Static analysis tool for @throws TSDoc declarations")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Check TypeScript files for missing @throws declarations
    Check {
        /// Files or directories to check
        #[arg(default_value = ".")]
        paths: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { paths }) => {
            println!("Checking: {:?}", paths);
        }
        None => {
            println!("Use --help for usage information");
        }
    }
}
```

- [ ] **Step 4: Verify crate compiles**

Run: `cargo build -p throw-trace`
Expected: Compiling throw-trace ... Finished

- [ ] **Step 5: Test CLI help**

Run: `cargo run -p throw-trace -- --help`
Expected: Shows "Static analysis tool for @throws TSDoc declarations" and check subcommand

- [ ] **Step 6: Commit**

```bash
git add crates/throw-trace/
git commit -m "$(cat <<'EOF'
feat(cli): add throw-trace CLI crate with check subcommand
EOF
)"
```

---

## Task 5: Core Types - FunctionId and Span

**Files:**
- Create: `crates/throw-trace-core/src/types.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Write the failing test in lib.rs**

```rust
//! Core engine for throw-trace: types, call graph, propagation analysis.

mod types;

pub use types::{FunctionId, Span};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn function_id_display() {
        let id = FunctionId {
            file_path: PathBuf::from("src/service.ts"),
            name: "createUser".into(),
            span: Span { start: 10, end: 50 },
        };
        assert_eq!(format!("{id}"), "src/service.ts:createUser");
    }

    #[test]
    fn function_id_anonymous() {
        let id = FunctionId::anonymous(PathBuf::from("src/util.ts"), 42, Span { start: 100, end: 150 });
        assert_eq!(id.name.as_str(), "anonymous_L42");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find module `types`"

- [ ] **Step 3: Create types.rs with implementation**

```rust
use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionId {
    pub file_path: PathBuf,
    pub name: CompactString,
    pub span: Span,
}

impl FunctionId {
    pub fn new(file_path: PathBuf, name: impl Into<CompactString>, span: Span) -> Self {
        Self {
            file_path,
            name: name.into(),
            span,
        }
    }

    pub fn anonymous(file_path: PathBuf, line: u32, span: Span) -> Self {
        Self {
            file_path,
            name: format!("anonymous_L{line}").into(),
            span,
        }
    }
}

impl fmt::Display for FunctionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file_path.display(), self.name)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: test tests::function_id_display ... ok, test tests::function_id_anonymous ... ok

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add FunctionId and Span types
EOF
)"
```

---

## Task 6: Core Types - ErrorType and ThrowSite

**Files:**
- Modify: `crates/throw-trace-core/src/types.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
pub use types::{ErrorType, FunctionId, Span, ThrowSite};

// 既存のテストの下に追加
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn function_id_display() {
        let id = FunctionId {
            file_path: PathBuf::from("src/service.ts"),
            name: "createUser".into(),
            span: Span { start: 10, end: 50 },
        };
        assert_eq!(format!("{id}"), "src/service.ts:createUser");
    }

    #[test]
    fn function_id_anonymous() {
        let id = FunctionId::anonymous(PathBuf::from("src/util.ts"), 42, Span { start: 100, end: 150 });
        assert_eq!(id.name.as_str(), "anonymous_L42");
    }

    #[test]
    fn error_type_named() {
        let err = ErrorType::Named("ValidationError".into());
        assert_eq!(err.type_name(), Some("ValidationError"));
    }

    #[test]
    fn error_type_unknown() {
        let err = ErrorType::Unknown;
        assert_eq!(err.type_name(), None);
    }

    #[test]
    fn throw_site_creation() {
        let site = ThrowSite {
            location: Span { start: 100, end: 120 },
            error_type: ErrorType::Named("MyError".into()),
        };
        assert_eq!(site.error_type.type_name(), Some("MyError"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find `ErrorType`"

- [ ] **Step 3: Add ErrorType and ThrowSite to types.rs**

```rust
use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionId {
    pub file_path: PathBuf,
    pub name: CompactString,
    pub span: Span,
}

impl FunctionId {
    pub fn new(file_path: PathBuf, name: impl Into<CompactString>, span: Span) -> Self {
        Self {
            file_path,
            name: name.into(),
            span,
        }
    }

    pub fn anonymous(file_path: PathBuf, line: u32, span: Span) -> Self {
        Self {
            file_path,
            name: format!("anonymous_L{line}").into(),
            span,
        }
    }
}

impl fmt::Display for FunctionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file_path.display(), self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorType {
    Named(CompactString),
    Unknown,
}

impl ErrorType {
    pub fn type_name(&self) -> Option<&str> {
        match self {
            Self::Named(name) => Some(name.as_str()),
            Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThrowSite {
    pub location: Span,
    pub error_type: ErrorType,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: All 5 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add ErrorType and ThrowSite types
EOF
)"
```

---

## Task 7: Core Types - DeclaredThrow and CallSite

**Files:**
- Modify: `crates/throw-trace-core/src/types.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
pub use types::{CallSite, DeclaredThrow, ErrorType, FunctionId, Span, ThrowSite};

// tests moduleに追加
    #[test]
    fn declared_throw_with_description() {
        let decl = DeclaredThrow {
            error_type: "ValidationError".into(),
            description: Some("When input is invalid".into()),
            span: Span { start: 5, end: 50 },
        };
        assert_eq!(decl.error_type.as_str(), "ValidationError");
        assert!(decl.description.is_some());
    }

    #[test]
    fn call_site_creation() {
        let call = CallSite {
            callee_name: "validate".into(),
            location: Span { start: 200, end: 220 },
        };
        assert_eq!(call.callee_name.as_str(), "validate");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find `DeclaredThrow`"

- [ ] **Step 3: Add DeclaredThrow and CallSite to types.rs**

types.rsの末尾に追加:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredThrow {
    pub error_type: CompactString,
    pub description: Option<CompactString>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSite {
    pub callee_name: CompactString,
    pub location: Span,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: All 7 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add DeclaredThrow and CallSite types
EOF
)"
```

---

## Task 8: Core Types - TryCatchBlock and FunctionSignature

**Files:**
- Modify: `crates/throw-trace-core/src/types.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
pub use types::{
    CallSite, DeclaredThrow, ErrorType, FunctionId, FunctionSignature, Span, ThrowSite,
    TryCatchBlock,
};

// tests moduleに追加
    #[test]
    fn try_catch_block_contains_span() {
        let block = TryCatchBlock {
            try_span: Span { start: 100, end: 200 },
            catch_span: Some(Span { start: 200, end: 300 }),
            caught_types: vec!["ValidationError".into()],
        };
        assert!(block.contains(150));
        assert!(!block.contains(50));
    }

    #[test]
    fn function_signature_creation() {
        let sig = FunctionSignature {
            id: FunctionId::new(
                PathBuf::from("src/test.ts"),
                "testFn",
                Span { start: 0, end: 100 },
            ),
            declared_throws: vec![],
            direct_throws: vec![],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        };
        assert_eq!(sig.id.name.as_str(), "testFn");
        assert!(!sig.is_async);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find `TryCatchBlock`"

- [ ] **Step 3: Add TryCatchBlock and FunctionSignature to types.rs**

types.rsの末尾に追加:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TryCatchBlock {
    pub try_span: Span,
    pub catch_span: Option<Span>,
    pub caught_types: Vec<CompactString>,
}

impl TryCatchBlock {
    pub fn contains(&self, offset: u32) -> bool {
        offset >= self.try_span.start && offset < self.try_span.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub id: FunctionId,
    pub declared_throws: Vec<DeclaredThrow>,
    pub direct_throws: Vec<ThrowSite>,
    pub calls: Vec<CallSite>,
    pub try_catch_blocks: Vec<TryCatchBlock>,
    pub is_async: bool,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: All 9 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add TryCatchBlock and FunctionSignature types
EOF
)"
```

---

## Task 9: Core Types - PropagatedThrow and Diagnostic

**Files:**
- Modify: `crates/throw-trace-core/src/types.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
pub use types::{
    CallSite, DeclaredThrow, Diagnostic, ErrorType, FunctionId, FunctionSignature,
    PropagatedThrow, Span, ThrowSite, TryCatchBlock,
};

// tests moduleに追加
    #[test]
    fn propagated_throw_path() {
        let origin = ThrowSite {
            location: Span { start: 10, end: 30 },
            error_type: ErrorType::Named("DBError".into()),
        };
        let propagated = PropagatedThrow {
            error_type: ErrorType::Named("DBError".into()),
            origin: origin.clone(),
            path: vec![
                FunctionId::new(PathBuf::from("a.ts"), "inner", Span { start: 0, end: 50 }),
                FunctionId::new(PathBuf::from("b.ts"), "outer", Span { start: 0, end: 100 }),
            ],
        };
        assert_eq!(propagated.path.len(), 2);
    }

    #[test]
    fn diagnostic_missing_throws() {
        let func_id = FunctionId::new(
            PathBuf::from("src/service.ts"),
            "createUser",
            Span { start: 0, end: 200 },
        );
        let diagnostic = Diagnostic {
            function: func_id,
            missing_throws: vec![PropagatedThrow {
                error_type: ErrorType::Named("ValidationError".into()),
                origin: ThrowSite {
                    location: Span { start: 50, end: 80 },
                    error_type: ErrorType::Named("ValidationError".into()),
                },
                path: vec![],
            }],
        };
        assert_eq!(diagnostic.missing_throws.len(), 1);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find `PropagatedThrow`"

- [ ] **Step 3: Add PropagatedThrow and Diagnostic to types.rs**

types.rsの末尾に追加:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropagatedThrow {
    pub error_type: ErrorType,
    pub origin: ThrowSite,
    pub path: Vec<FunctionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub function: FunctionId,
    pub missing_throws: Vec<PropagatedThrow>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: All 11 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add PropagatedThrow and Diagnostic types
EOF
)"
```

---

## Task 10: TS Parser - Basic oxc Wrapper

**Files:**
- Create: `crates/throw-trace-ts/src/parser.rs`
- Modify: `crates/throw-trace-ts/src/lib.rs`

- [ ] **Step 1: Write the failing test in lib.rs**

```rust
//! TypeScript adapter for throw-trace (oxc-based).

mod parser;

pub use parser::parse_source;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_source_returns_program() {
        let source = "function foo() { return 1; }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_handles_typescript() {
        let source = "function foo(x: number): string { return x.toString(); }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_reports_syntax_error() {
        let source = "function foo( { }";
        let result = parse_source(source);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-ts`
Expected: FAIL with "cannot find module `parser`"

- [ ] **Step 3: Create parser.rs with implementation**

```rust
use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Syntax error: {0}")]
    SyntaxError(String),
}

pub fn parse_source(source: &str) -> Result<(), ParseError> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return: ParserReturn = Parser::new(&allocator, source, source_type).parse();

    if parser_return.errors.is_empty() {
        Ok(())
    } else {
        let msg = parser_return
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        Err(ParseError::SyntaxError(msg))
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-ts`
Expected: All 3 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-ts/src/
git commit -m "$(cat <<'EOF'
feat(ts): add oxc parser wrapper
EOF
)"
```

---

## Task 11: TS Parser - Function Extraction

**Files:**
- Create: `crates/throw-trace-ts/src/extract.rs`
- Modify: `crates/throw-trace-ts/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
//! TypeScript adapter for throw-trace (oxc-based).

mod extract;
mod parser;

pub use extract::extract_functions;
pub use parser::parse_source;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_source_returns_program() {
        let source = "function foo() { return 1; }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_handles_typescript() {
        let source = "function foo(x: number): string { return x.toString(); }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_reports_syntax_error() {
        let source = "function foo( { }";
        let result = parse_source(source);
        assert!(result.is_err());
    }

    #[test]
    fn extract_functions_finds_function_declaration() {
        let source = "function foo() { }\nfunction bar() { }";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 2);
        assert_eq!(sigs[0].id.name.as_str(), "foo");
        assert_eq!(sigs[1].id.name.as_str(), "bar");
    }

    #[test]
    fn extract_functions_finds_arrow_function() {
        let source = "const add = (a: number, b: number) => a + b;";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].id.name.as_str(), "add");
    }

    #[test]
    fn extract_functions_finds_async_function() {
        let source = "async function fetchData() { }";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].is_async);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-ts`
Expected: FAIL with "cannot find module `extract`"

- [ ] **Step 3: Create extract.rs with implementation**

```rust
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, BindingPatternKind, Declaration, Function, Statement,
    VariableDeclarationKind,
};
use oxc_ast::visit::walk;
use oxc_ast::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;
use throw_trace_core::{FunctionSignature, FunctionId, Span};

use crate::parser::ParseError;

struct FunctionExtractor<'a> {
    file_path: &'a Path,
    signatures: Vec<FunctionSignature>,
}

impl<'a> FunctionExtractor<'a> {
    fn new(file_path: &'a Path) -> Self {
        Self {
            file_path,
            signatures: Vec::new(),
        }
    }

    fn add_function(&mut self, name: &str, span: oxc_span::Span, is_async: bool) {
        let id = FunctionId::new(
            self.file_path.to_path_buf(),
            name,
            Span {
                start: span.start,
                end: span.end,
            },
        );
        self.signatures.push(FunctionSignature {
            id,
            declared_throws: Vec::new(),
            direct_throws: Vec::new(),
            calls: Vec::new(),
            try_catch_blocks: Vec::new(),
            is_async,
        });
    }
}

impl<'a> Visit<'a> for FunctionExtractor<'a> {
    fn visit_function(&mut self, func: &Function<'a>, _flags: oxc_ast::ast::ScopeFlags) {
        if let Some(id) = &func.id {
            self.add_function(id.name.as_str(), func.span, func.r#async);
        }
        walk::walk_function(self, func, _flags);
    }

    fn visit_variable_declaration(&mut self, decl: &oxc_ast::ast::VariableDeclaration<'a>) {
        for declarator in &decl.declarations {
            if let Some(init) = &declarator.init {
                if let oxc_ast::ast::Expression::ArrowFunctionExpression(arrow) = init {
                    if let BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind {
                        self.add_function(id.name.as_str(), arrow.span, arrow.r#async);
                    }
                }
            }
        }
        walk::walk_variable_declaration(self, decl);
    }
}

pub fn extract_functions(
    source: &str,
    file_path: &Path,
) -> Result<Vec<FunctionSignature>, ParseError> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    if !parser_return.errors.is_empty() {
        let msg = parser_return
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ParseError::SyntaxError(msg));
    }

    let mut extractor = FunctionExtractor::new(file_path);
    extractor.visit_program(&parser_return.program);

    Ok(extractor.signatures)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-ts`
Expected: All 6 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-ts/src/
git commit -m "$(cat <<'EOF'
feat(ts): add function extraction from AST
EOF
)"
```

---

## Task 12: TS Parser - JSDoc @throws Extraction

**Files:**
- Create: `crates/throw-trace-ts/src/jsdoc.rs`
- Modify: `crates/throw-trace-ts/src/lib.rs`
- Modify: `crates/throw-trace-ts/src/extract.rs` (JSDoc統合)

- [ ] **Step 1: Add tests to lib.rs**

```rust
mod jsdoc;

pub use jsdoc::extract_throws_from_jsdoc;

// testsに追加
    #[test]
    fn extract_throws_single() {
        let comment = "/**\n * @throws {ValidationError} When input is invalid\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].0, "ValidationError");
        assert_eq!(throws[0].1.as_deref(), Some("When input is invalid"));
    }

    #[test]
    fn extract_throws_multiple() {
        let comment = "/**\n * @throws {ValidationError}\n * @throws {NetworkError} Connection failed\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].0, "ValidationError");
        assert_eq!(throws[1].0, "NetworkError");
    }

    #[test]
    fn extract_throws_no_braces() {
        let comment = "/** @throws Error when something fails */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].0, "Error");
    }

    #[test]
    fn extract_throws_empty() {
        let comment = "/** This is a description */";
        let throws = extract_throws_from_jsdoc(comment);
        assert!(throws.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-ts`
Expected: FAIL with "cannot find module `jsdoc`"

- [ ] **Step 3: Create jsdoc.rs with implementation**

```rust
/// Extract @throws declarations from a JSDoc comment string.
/// Returns Vec of (type_name, optional_description).
pub fn extract_throws_from_jsdoc(comment: &str) -> Vec<(String, Option<String>)> {
    let mut results = Vec::new();

    for line in comment.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if !trimmed.starts_with("@throws") {
            continue;
        }

        let rest = trimmed.strip_prefix("@throws").unwrap_or("").trim();

        if rest.starts_with('{') {
            if let Some(end_brace) = rest.find('}') {
                let type_name = rest[1..end_brace].trim().to_string();
                let description = rest[end_brace + 1..].trim();
                let desc = if description.is_empty() {
                    None
                } else {
                    Some(description.to_string())
                };
                results.push((type_name, desc));
            }
        } else {
            let parts: Vec<&str> = rest.splitn(2, char::is_whitespace).collect();
            if !parts.is_empty() && !parts[0].is_empty() {
                let type_name = parts[0].to_string();
                let description = parts.get(1).map(|s| s.trim().to_string());
                results.push((type_name, description));
            }
        }
    }

    results
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-ts`
Expected: All 10 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-ts/src/
git commit -m "$(cat <<'EOF'
feat(ts): add JSDoc @throws extraction
EOF
)"
```

---

## Task 13: TS Parser - Throw Statement Detection

**Files:**
- Create: `crates/throw-trace-ts/src/throw_analyzer.rs`
- Modify: `crates/throw-trace-ts/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
mod throw_analyzer;

pub use throw_analyzer::analyze_throw_expr;

// testsに追加
    use throw_trace_core::ErrorType;

    #[test]
    fn analyze_throw_new_error() {
        let source = "throw new ValidationError('msg')";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Named("ValidationError".into()));
    }

    #[test]
    fn analyze_throw_new_error_simple() {
        let source = "throw new Error('msg')";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Named("Error".into()));
    }

    #[test]
    fn analyze_throw_literal() {
        let source = "throw 'error'";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Unknown);
    }

    #[test]
    fn analyze_throw_variable() {
        let source = "throw err";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Unknown);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-ts`
Expected: FAIL with "cannot find module `throw_analyzer`"

- [ ] **Step 3: Create throw_analyzer.rs with implementation**

```rust
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use throw_trace_core::ErrorType;

/// Analyze a throw expression and extract the error type.
/// Input should be a throw statement like "throw new Error('msg')".
pub fn analyze_throw_expr(source: &str) -> ErrorType {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    if !parser_return.errors.is_empty() {
        return ErrorType::Unknown;
    }

    let program = &parser_return.program;
    if program.body.is_empty() {
        return ErrorType::Unknown;
    }

    if let Statement::ThrowStatement(throw_stmt) = &program.body[0] {
        return analyze_expression(&throw_stmt.argument);
    }

    ErrorType::Unknown
}

fn analyze_expression(expr: &Expression) -> ErrorType {
    match expr {
        Expression::NewExpression(new_expr) => {
            if let Expression::Identifier(id) = &new_expr.callee {
                return ErrorType::Named(id.name.as_str().into());
            }
            ErrorType::Unknown
        }
        Expression::CallExpression(call_expr) => {
            if let Expression::Identifier(id) = &call_expr.callee {
                if id.name.as_str().ends_with("Error") {
                    return ErrorType::Named(id.name.as_str().into());
                }
            }
            ErrorType::Unknown
        }
        Expression::ConditionalExpression(cond) => {
            let consequent = analyze_expression(&cond.consequent);
            if matches!(consequent, ErrorType::Named(_)) {
                return consequent;
            }
            analyze_expression(&cond.alternate)
        }
        _ => ErrorType::Unknown,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-ts`
Expected: All 14 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-ts/src/
git commit -m "$(cat <<'EOF'
feat(ts): add throw expression type analysis
EOF
)"
```

---

## Task 14: TS Parser - Try-Catch Block Detection

**Files:**
- Create: `crates/throw-trace-ts/src/try_catch.rs`
- Modify: `crates/throw-trace-ts/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
mod try_catch;

pub use try_catch::extract_try_catch_blocks;

// testsに追加
    use throw_trace_core::TryCatchBlock;

    #[test]
    fn extract_try_catch_simple() {
        let source = r#"
try {
    validate();
} catch (e) {
    console.log(e);
}
"#;
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].catch_span.is_some());
    }

    #[test]
    fn extract_try_catch_with_instanceof() {
        let source = r#"
try {
    validate();
} catch (e) {
    if (e instanceof ValidationError) {
        return;
    }
    throw e;
}
"#;
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].caught_types.len(), 1);
        assert_eq!(blocks[0].caught_types[0].as_str(), "ValidationError");
    }

    #[test]
    fn extract_try_catch_no_catch() {
        let source = r#"
try {
    validate();
} finally {
    cleanup();
}
"#;
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].catch_span.is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-ts`
Expected: FAIL with "cannot find module `try_catch`"

- [ ] **Step 3: Create try_catch.rs with implementation**

```rust
use compact_str::CompactString;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
use oxc_ast::visit::walk;
use oxc_ast::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;
use throw_trace_core::{Span, TryCatchBlock};

struct TryCatchExtractor {
    blocks: Vec<TryCatchBlock>,
}

impl TryCatchExtractor {
    fn new() -> Self {
        Self { blocks: Vec::new() }
    }
}

impl<'a> Visit<'a> for TryCatchExtractor {
    fn visit_try_statement(&mut self, stmt: &oxc_ast::ast::TryStatement<'a>) {
        let try_span = Span {
            start: stmt.block.span.start,
            end: stmt.block.span.end,
        };

        let (catch_span, caught_types) = if let Some(handler) = &stmt.handler {
            let span = Some(Span {
                start: handler.span.start,
                end: handler.span.end,
            });
            let types = extract_instanceof_checks(&handler.body);
            (span, types)
        } else {
            (None, Vec::new())
        };

        self.blocks.push(TryCatchBlock {
            try_span,
            catch_span,
            caught_types,
        });

        walk::walk_try_statement(self, stmt);
    }
}

fn extract_instanceof_checks(block: &oxc_ast::ast::BlockStatement) -> Vec<CompactString> {
    let mut types = Vec::new();

    for stmt in &block.body {
        if let Statement::IfStatement(if_stmt) = stmt {
            if let Expression::BinaryExpression(bin) = &if_stmt.test {
                if bin.operator == oxc_ast::ast::BinaryOperator::Instanceof {
                    if let Expression::Identifier(id) = &bin.right {
                        types.push(id.name.as_str().into());
                    }
                }
            }
        }
    }

    types
}

pub fn extract_try_catch_blocks(source: &str) -> Vec<TryCatchBlock> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    if !parser_return.errors.is_empty() {
        return Vec::new();
    }

    let mut extractor = TryCatchExtractor::new();
    extractor.visit_program(&parser_return.program);

    extractor.blocks
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-ts`
Expected: All 17 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-ts/src/
git commit -m "$(cat <<'EOF'
feat(ts): add try-catch block detection with instanceof analysis
EOF
)"
```

---

## Task 15: Core - Call Graph Structure

**Files:**
- Create: `crates/throw-trace-core/src/call_graph.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
mod call_graph;

pub use call_graph::CallGraph;

// testsに追加
    #[test]
    fn call_graph_add_function() {
        let mut graph = CallGraph::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });
        graph.add_function(id.clone());
        assert!(graph.contains(&id));
    }

    #[test]
    fn call_graph_add_call() {
        let mut graph = CallGraph::new();
        let caller = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });
        let callee = FunctionId::new(PathBuf::from("b.ts"), "bar", Span { start: 0, end: 50 });
        graph.add_function(caller.clone());
        graph.add_function(callee.clone());
        graph.add_call(&caller, &callee);
        let callees = graph.get_callees(&caller);
        assert_eq!(callees.len(), 1);
    }

    #[test]
    fn call_graph_transitive_callees() {
        let mut graph = CallGraph::new();
        let a = FunctionId::new(PathBuf::from("a.ts"), "a", Span { start: 0, end: 50 });
        let b = FunctionId::new(PathBuf::from("b.ts"), "b", Span { start: 0, end: 50 });
        let c = FunctionId::new(PathBuf::from("c.ts"), "c", Span { start: 0, end: 50 });
        graph.add_function(a.clone());
        graph.add_function(b.clone());
        graph.add_function(c.clone());
        graph.add_call(&a, &b);
        graph.add_call(&b, &c);
        let all_callees = graph.get_transitive_callees(&a);
        assert_eq!(all_callees.len(), 2);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find module `call_graph`"

- [ ] **Step 3: Create call_graph.rs with implementation**

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: All 14 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add CallGraph with petgraph
EOF
)"
```

---

## Task 16: Core - Throw Propagation Logic

**Files:**
- Create: `crates/throw-trace-core/src/propagation.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
mod propagation;

pub use propagation::compute_propagated_throws;

// testsに追加
    #[test]
    fn propagation_direct_throw() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });
        let sig = FunctionSignature {
            id: id.clone(),
            declared_throws: vec![],
            direct_throws: vec![ThrowSite {
                location: Span { start: 10, end: 30 },
                error_type: ErrorType::Named("MyError".into()),
            }],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        };
        signatures.insert(id.clone(), sig);

        let graph = CallGraph::new();
        let propagated = compute_propagated_throws(&id, &signatures, &graph);
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].error_type, ErrorType::Named("MyError".into()));
    }

    #[test]
    fn propagation_from_callee() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let mut graph = CallGraph::new();

        let inner = FunctionId::new(PathBuf::from("a.ts"), "inner", Span { start: 0, end: 50 });
        let outer = FunctionId::new(PathBuf::from("b.ts"), "outer", Span { start: 0, end: 100 });

        signatures.insert(inner.clone(), FunctionSignature {
            id: inner.clone(),
            declared_throws: vec![],
            direct_throws: vec![ThrowSite {
                location: Span { start: 10, end: 30 },
                error_type: ErrorType::Named("InnerError".into()),
            }],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        });

        signatures.insert(outer.clone(), FunctionSignature {
            id: outer.clone(),
            declared_throws: vec![],
            direct_throws: vec![],
            calls: vec![CallSite {
                callee_name: "inner".into(),
                location: Span { start: 50, end: 60 },
            }],
            try_catch_blocks: vec![],
            is_async: false,
        });

        graph.add_function(inner.clone());
        graph.add_function(outer.clone());
        graph.add_call(&outer, &inner);

        let propagated = compute_propagated_throws(&outer, &signatures, &graph);
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].error_type, ErrorType::Named("InnerError".into()));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find module `propagation`"

- [ ] **Step 3: Create propagation.rs with implementation**

```rust
use crate::{CallGraph, ErrorType, FunctionId, FunctionSignature, PropagatedThrow, ThrowSite};
use std::collections::{HashMap, HashSet};

pub fn compute_propagated_throws(
    func_id: &FunctionId,
    signatures: &HashMap<FunctionId, FunctionSignature>,
    graph: &CallGraph,
) -> Vec<PropagatedThrow> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();

    collect_throws(func_id, signatures, graph, &mut result, &mut visited, vec![]);

    result
}

fn collect_throws(
    func_id: &FunctionId,
    signatures: &HashMap<FunctionId, FunctionSignature>,
    graph: &CallGraph,
    result: &mut Vec<PropagatedThrow>,
    visited: &mut HashSet<FunctionId>,
    path: Vec<FunctionId>,
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
                path: path.clone(),
            });
        }
    }

    for callee_id in graph.get_callees(func_id) {
        let mut new_path = path.clone();
        new_path.push(func_id.clone());
        collect_throws(&callee_id, signatures, graph, result, visited, new_path);
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
```

- [ ] **Step 4: Update lib.rs imports**

```rust
//! Core engine for throw-trace: types, call graph, propagation analysis.

mod call_graph;
mod propagation;
mod types;

pub use call_graph::CallGraph;
pub use propagation::compute_propagated_throws;
pub use types::{
    CallSite, DeclaredThrow, Diagnostic, ErrorType, FunctionId, FunctionSignature,
    PropagatedThrow, Span, ThrowSite, TryCatchBlock,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    // ... all tests ...
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: All 16 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add throw propagation analysis
EOF
)"
```

---

## Task 17: Core - Diagnostic Generation

**Files:**
- Create: `crates/throw-trace-core/src/diagnostic.rs`
- Modify: `crates/throw-trace-core/src/lib.rs`

- [ ] **Step 1: Add tests to lib.rs**

```rust
mod diagnostic;

pub use diagnostic::generate_diagnostics;

// testsに追加
    #[test]
    fn diagnostic_missing_declaration() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });

        signatures.insert(id.clone(), FunctionSignature {
            id: id.clone(),
            declared_throws: vec![],
            direct_throws: vec![ThrowSite {
                location: Span { start: 10, end: 30 },
                error_type: ErrorType::Named("MyError".into()),
            }],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        });

        let graph = CallGraph::new();
        let diagnostics = generate_diagnostics(&signatures, &graph);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].missing_throws.len(), 1);
    }

    #[test]
    fn diagnostic_declared_ok() {
        let mut signatures: HashMap<FunctionId, FunctionSignature> = HashMap::new();
        let id = FunctionId::new(PathBuf::from("a.ts"), "foo", Span { start: 0, end: 50 });

        signatures.insert(id.clone(), FunctionSignature {
            id: id.clone(),
            declared_throws: vec![DeclaredThrow {
                error_type: "MyError".into(),
                description: None,
                span: Span { start: 0, end: 10 },
            }],
            direct_throws: vec![ThrowSite {
                location: Span { start: 10, end: 30 },
                error_type: ErrorType::Named("MyError".into()),
            }],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        });

        let graph = CallGraph::new();
        let diagnostics = generate_diagnostics(&signatures, &graph);
        assert!(diagnostics.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p throw-trace-core`
Expected: FAIL with "cannot find module `diagnostic`"

- [ ] **Step 3: Create diagnostic.rs with implementation**

```rust
use crate::{
    compute_propagated_throws, CallGraph, Diagnostic, ErrorType, FunctionId, FunctionSignature,
    PropagatedThrow,
};
use std::collections::HashMap;

pub fn generate_diagnostics(
    signatures: &HashMap<FunctionId, FunctionSignature>,
    graph: &CallGraph,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (func_id, sig) in signatures {
        let propagated = compute_propagated_throws(func_id, signatures, graph);
        let missing = find_missing_declarations(sig, &propagated);

        if !missing.is_empty() {
            diagnostics.push(Diagnostic {
                function: func_id.clone(),
                missing_throws: missing,
            });
        }
    }

    diagnostics
}

fn find_missing_declarations(
    sig: &FunctionSignature,
    propagated: &[PropagatedThrow],
) -> Vec<PropagatedThrow> {
    propagated
        .iter()
        .filter(|p| !is_declared(&p.error_type, sig))
        .cloned()
        .collect()
}

fn is_declared(error_type: &ErrorType, sig: &FunctionSignature) -> bool {
    let ErrorType::Named(type_name) = error_type else {
        return false;
    };

    sig.declared_throws
        .iter()
        .any(|d| d.error_type.as_str() == type_name.as_str())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p throw-trace-core`
Expected: All 18 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/throw-trace-core/src/
git commit -m "$(cat <<'EOF'
feat(core): add diagnostic generation for missing @throws
EOF
)"
```

---

## Task 18: CLI - File Loader

**Files:**
- Create: `crates/throw-trace/src/loader.rs`
- Modify: `crates/throw-trace/src/main.rs`

- [ ] **Step 1: Create loader.rs**

```rust
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct FileLoader {
    exclude_patterns: GlobSet,
}

impl FileLoader {
    pub fn new(exclude_patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for pattern in exclude_patterns {
            builder.add(Glob::new(pattern).context("Invalid glob pattern")?);
        }
        Ok(Self {
            exclude_patterns: builder.build()?,
        })
    }

    pub fn collect_ts_files(&self, paths: &[String]) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for path_str in paths {
            let path = Path::new(path_str);

            if path.is_file() {
                if self.is_ts_file(path) && !self.is_excluded(path) {
                    files.push(path.to_path_buf());
                }
            } else if path.is_dir() {
                for entry in WalkBuilder::new(path).build() {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if entry_path.is_file()
                        && self.is_ts_file(entry_path)
                        && !self.is_excluded(entry_path)
                    {
                        files.push(entry_path.to_path_buf());
                    }
                }
            }
        }

        Ok(files)
    }

    fn is_ts_file(&self, path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ts") | Some("tsx") | Some("mts") | Some("cts")
        )
    }

    fn is_excluded(&self, path: &Path) -> bool {
        self.exclude_patterns.is_match(path)
    }
}
```

- [ ] **Step 2: Update main.rs to use loader**

```rust
mod loader;

use anyhow::Result;
use clap::Parser;
use loader::FileLoader;

#[derive(Parser)]
#[command(name = "throw-trace")]
#[command(about = "Static analysis tool for @throws TSDoc declarations")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Check TypeScript files for missing @throws declarations
    Check {
        /// Files or directories to check
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Exclude patterns (glob)
        #[arg(long, short = 'e')]
        exclude: Vec<String>,

        /// Output format
        #[arg(long, short = 'f', default_value = "text")]
        format: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check {
            paths,
            exclude,
            format,
        }) => {
            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;
            println!("Found {} TypeScript files", files.len());
            for file in &files {
                println!("  {}", file.display());
            }
            Ok(())
        }
        None => {
            println!("Use --help for usage information");
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Verify crate compiles**

Run: `cargo build -p throw-trace`
Expected: Compiling throw-trace ... Finished

- [ ] **Step 4: Commit**

```bash
git add crates/throw-trace/src/
git commit -m "$(cat <<'EOF'
feat(cli): add file loader with glob exclude support
EOF
)"
```

---

## Task 19: CLI - Analyzer Integration

**Files:**
- Create: `crates/throw-trace/src/analyzer.rs`
- Modify: `crates/throw-trace/src/main.rs`

- [ ] **Step 1: Create analyzer.rs**

```rust
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
        Self {
            signatures: HashMap::new(),
            graph: CallGraph::new(),
        }
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
        let sig_map: HashMap<&str, &FunctionId> = self
            .signatures
            .values()
            .map(|sig| (sig.id.name.as_str(), &sig.id))
            .collect();

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
```

- [ ] **Step 2: Update main.rs to use analyzer**

```rust
mod analyzer;
mod loader;

use analyzer::Analyzer;
use anyhow::Result;
use clap::Parser;
use loader::FileLoader;

#[derive(Parser)]
#[command(name = "throw-trace")]
#[command(about = "Static analysis tool for @throws TSDoc declarations")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Check TypeScript files for missing @throws declarations
    Check {
        /// Files or directories to check
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Exclude patterns (glob)
        #[arg(long, short = 'e')]
        exclude: Vec<String>,

        /// Output format
        #[arg(long, short = 'f', default_value = "text")]
        format: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check {
            paths,
            exclude,
            format,
        }) => {
            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;

            if files.is_empty() {
                println!("No TypeScript files found");
                return Ok(());
            }

            let mut analyzer = Analyzer::new();
            analyzer.analyze_files(&files)?;

            let diagnostics = analyzer.generate_diagnostics();

            if diagnostics.is_empty() {
                println!("No issues found");
            } else {
                println!("Found {} issues", diagnostics.len());
                for diag in &diagnostics {
                    println!("  {}: missing @throws", diag.function);
                }
            }

            Ok(())
        }
        None => {
            println!("Use --help for usage information");
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Verify crate compiles**

Run: `cargo build -p throw-trace`
Expected: Compiling throw-trace ... Finished

- [ ] **Step 4: Commit**

```bash
git add crates/throw-trace/src/
git commit -m "$(cat <<'EOF'
feat(cli): add analyzer integration
EOF
)"
```

---

## Task 20: CLI - Reporter (text/json output)

**Files:**
- Create: `crates/throw-trace/src/reporter.rs`
- Modify: `crates/throw-trace/src/main.rs`

- [ ] **Step 1: Create reporter.rs**

```rust
use serde::Serialize;
use std::io::{self, Write};
use throw_trace_core::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Serialize)]
struct JsonReport {
    diagnostics: Vec<JsonDiagnostic>,
    summary: Summary,
}

#[derive(Serialize)]
struct JsonDiagnostic {
    file: String,
    function: String,
    missing_throws: Vec<JsonMissingThrow>,
}

#[derive(Serialize)]
struct JsonMissingThrow {
    error_type: String,
    origin_file: String,
    origin_line: u32,
}

#[derive(Serialize)]
struct Summary {
    errors: usize,
    files_checked: usize,
}

pub fn report(
    diagnostics: &[Diagnostic],
    files_checked: usize,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Text => report_text(diagnostics, files_checked),
        OutputFormat::Json => report_json(diagnostics, files_checked),
    }
}

fn report_text(diagnostics: &[Diagnostic], files_checked: usize) -> io::Result<()> {
    let mut stdout = io::stdout().lock();

    if diagnostics.is_empty() {
        writeln!(stdout, "No issues found in {} files", files_checked)?;
        return Ok(());
    }

    for diag in diagnostics {
        writeln!(stdout, "error: missing @throws declaration")?;
        writeln!(stdout, "  --> {}:{}", diag.function.file_path.display(), diag.function.name)?;
        writeln!(stdout, "   |")?;

        for missing in &diag.missing_throws {
            let type_name = missing
                .error_type
                .type_name()
                .unwrap_or("Unknown");
            writeln!(
                stdout,
                "   | {} propagates from {:?}",
                type_name,
                missing.origin.location
            )?;
        }

        writeln!(stdout, "   |")?;
        let types: Vec<_> = diag
            .missing_throws
            .iter()
            .filter_map(|m| m.error_type.type_name())
            .collect();
        writeln!(
            stdout,
            "   = help: add @throws {{{}}} to function {}",
            types.join(", "),
            diag.function.name
        )?;
        writeln!(stdout)?;
    }

    let error_count: usize = diagnostics.iter().map(|d| d.missing_throws.len()).sum();
    writeln!(
        stdout,
        "Found {} errors in {} files",
        error_count,
        files_checked
    )?;

    Ok(())
}

fn report_json(diagnostics: &[Diagnostic], files_checked: usize) -> io::Result<()> {
    let json_diagnostics: Vec<JsonDiagnostic> = diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            file: d.function.file_path.display().to_string(),
            function: d.function.name.to_string(),
            missing_throws: d
                .missing_throws
                .iter()
                .map(|m| JsonMissingThrow {
                    error_type: m.error_type.type_name().unwrap_or("Unknown").to_string(),
                    origin_file: String::new(),
                    origin_line: m.origin.location.start,
                })
                .collect(),
        })
        .collect();

    let error_count: usize = diagnostics.iter().map(|d| d.missing_throws.len()).sum();

    let report = JsonReport {
        diagnostics: json_diagnostics,
        summary: Summary {
            errors: error_count,
            files_checked,
        },
    };

    let json = serde_json::to_string_pretty(&report)?;
    println!("{json}");

    Ok(())
}
```

- [ ] **Step 2: Update main.rs to use reporter**

```rust
mod analyzer;
mod loader;
mod reporter;

use analyzer::Analyzer;
use anyhow::{bail, Result};
use clap::Parser;
use loader::FileLoader;
use reporter::{report, OutputFormat};

#[derive(Parser)]
#[command(name = "throw-trace")]
#[command(about = "Static analysis tool for @throws TSDoc declarations")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Check TypeScript files for missing @throws declarations
    Check {
        /// Files or directories to check
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Exclude patterns (glob)
        #[arg(long, short = 'e')]
        exclude: Vec<String>,

        /// Output format (text or json)
        #[arg(long, short = 'f', default_value = "text")]
        format: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check {
            paths,
            exclude,
            format,
        }) => {
            let output_format = OutputFormat::from_str(&format)
                .ok_or_else(|| anyhow::anyhow!("Invalid format: {format}"))?;

            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;

            if files.is_empty() {
                println!("No TypeScript files found");
                return Ok(());
            }

            let mut analyzer = Analyzer::new();
            analyzer.analyze_files(&files)?;

            let diagnostics = analyzer.generate_diagnostics();

            report(&diagnostics, files.len(), output_format)?;

            if !diagnostics.is_empty() {
                std::process::exit(1);
            }

            Ok(())
        }
        None => {
            println!("Use --help for usage information");
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Verify crate compiles**

Run: `cargo build -p throw-trace`
Expected: Compiling throw-trace ... Finished

- [ ] **Step 4: Commit**

```bash
git add crates/throw-trace/src/
git commit -m "$(cat <<'EOF'
feat(cli): add text and JSON reporter
EOF
)"
```

---

## Task 21: Test Fixtures and Integration Test

**Files:**
- Create: `tests/fixtures/simple_throw.ts`
- Create: `tests/fixtures/propagation.ts`
- Create: `tests/fixtures/try_catch.ts`
- Create: `crates/throw-trace/tests/integration_test.rs`

- [ ] **Step 1: Create test fixtures directory**

Run: `mkdir -p tests/fixtures`

- [ ] **Step 2: Create simple_throw.ts**

```typescript
// tests/fixtures/simple_throw.ts

// Missing @throws - should report error
function validate(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

/**
 * @throws {ValidationError} When input is invalid
 */
function validateWithDoc(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
  }
}
```

- [ ] **Step 3: Create propagation.ts**

```typescript
// tests/fixtures/propagation.ts

/**
 * @throws {DBError} When database fails
 */
function dbQuery() {
  throw new DBError("Connection failed");
}

// Missing @throws {DBError} - should report error (propagation)
function getUser(id: string) {
  return dbQuery();
}

/**
 * @throws {DBError} When database fails
 */
function getUserWithDoc(id: string) {
  return dbQuery();
}

class DBError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DBError";
  }
}
```

- [ ] **Step 4: Create try_catch.ts**

```typescript
// tests/fixtures/try_catch.ts

/**
 * @throws {ValidationError} When validation fails
 */
function validate() {
  throw new ValidationError("Invalid");
}

// No @throws needed - ValidationError is caught
function safeValidate() {
  try {
    validate();
  } catch (e) {
    if (e instanceof ValidationError) {
      return null;
    }
    throw e;
  }
}

// Missing @throws {ValidationError} - not caught
function unsafeValidate() {
  try {
    validate();
  } catch (e) {
    console.log(e);
    throw e;
  }
}

class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
  }
}
```

- [ ] **Step 5: Create integration test**

```rust
// crates/throw-trace/tests/integration_test.rs

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn check_simple_throw_reports_missing() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.args(["check", "tests/fixtures/simple_throw.ts"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));
}

#[test]
fn check_with_json_format() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.args(["check", "tests/fixtures/simple_throw.ts", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"diagnostics\""));
}

#[test]
fn check_nonexistent_path() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.args(["check", "nonexistent/path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No TypeScript files found"));
}
```

- [ ] **Step 6: Run integration tests**

Run: `cargo test -p throw-trace --test integration_test`
Expected: Tests may fail initially (depends on full implementation)

- [ ] **Step 7: Commit**

```bash
git add tests/ crates/throw-trace/tests/
git commit -m "$(cat <<'EOF'
test: add test fixtures and integration tests
EOF
)"
```

---

## Task 22: Final Integration and Polish

**Files:**
- Modify: Various files for final integration

- [ ] **Step 1: Run all tests**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 4: Build release**

Run: `cargo build --release -p throw-trace`
Expected: Build succeeds

- [ ] **Step 5: Test CLI manually**

Run: `./target/release/throw-trace check tests/fixtures/`
Expected: Reports missing @throws declarations

- [ ] **Step 6: Commit final polish**

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore: final integration and polish
EOF
)"
```

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | Workspace Setup | Cargo.toml, rust-toolchain.toml, rustfmt.toml, .gitignore |
| 2 | throw-trace-core Setup | crates/throw-trace-core/Cargo.toml, lib.rs |
| 3 | throw-trace-ts Setup | crates/throw-trace-ts/Cargo.toml, lib.rs |
| 4 | throw-trace CLI Setup | crates/throw-trace/Cargo.toml, main.rs |
| 5 | Core Types - FunctionId, Span | types.rs |
| 6 | Core Types - ErrorType, ThrowSite | types.rs |
| 7 | Core Types - DeclaredThrow, CallSite | types.rs |
| 8 | Core Types - TryCatchBlock, FunctionSignature | types.rs |
| 9 | Core Types - PropagatedThrow, Diagnostic | types.rs |
| 10 | TS Parser - oxc Wrapper | parser.rs |
| 11 | TS Parser - Function Extraction | extract.rs |
| 12 | TS Parser - JSDoc @throws | jsdoc.rs |
| 13 | TS Parser - Throw Statement Detection | throw_analyzer.rs |
| 14 | TS Parser - Try-Catch Detection | try_catch.rs |
| 15 | Core - Call Graph | call_graph.rs |
| 16 | Core - Propagation Logic | propagation.rs |
| 17 | Core - Diagnostic Generation | diagnostic.rs |
| 18 | CLI - File Loader | loader.rs |
| 19 | CLI - Analyzer Integration | analyzer.rs |
| 20 | CLI - Reporter | reporter.rs |
| 21 | Test Fixtures and Integration | tests/fixtures/, integration_test.rs |
| 22 | Final Integration | Polish and verification |

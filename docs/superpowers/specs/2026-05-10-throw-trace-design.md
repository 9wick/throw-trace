# throw-trace 設計仕様書

TypeScript向け`@throws` TSDoc静的解析ツール。関数が投げる可能性のある例外を追跡し、`@throws`宣言の漏れを検出する。

## 背景・課題

- Result型（neverthrow等）はROP向きだが、fw/libのAPIとしてユーザーに強要するのは不適切
- `@throws` TSDocをヌケモレなく書ける仕組みが必要
- 既存ツールは存在しない（eslint-plugin-jsdocは直接throwのみ、再帰追跡なし）

## 技術スタック

| 項目 | 選定 |
|------|------|
| 言語 | Rust |
| パーサー | Oxc（tsdown/Rolldown/Viteと同じ基盤） |
| call graph | petgraph |

## プロジェクト構造

```
throw-trace/
├── Cargo.toml              # workspace root
├── crates/
│   ├── throw-trace-core/   # 言語非依存: call graph, 伝播計算, レポート生成
│   ├── throw-trace-ts/     # TypeScript: oxcパーサー, AST→IR変換
│   └── throw-trace/        # CLI: clap, 出力フォーマット
```

将来の多言語対応を見据えた構造（layer-conformと同様）。

## コアデータ構造

```rust
// 関数の識別子
struct FunctionId {
    file_path: PathBuf,
    name: String,           // 関数名 or "anonymous_L42"
    span: Span,             // 位置情報
}

// throw情報
struct ThrowSite {
    location: Span,
    error_type: ErrorType,
}

enum ErrorType {
    Named(String),          // throw new ValidationError()
    Unknown,                // throw expr （型不明）
}

// 関数のシグネチャ（Pass1で収集）
struct FunctionSignature {
    id: FunctionId,
    declared_throws: Vec<DeclaredThrow>,  // @throws から抽出
    direct_throws: Vec<ThrowSite>,        // 直接のthrow文
    calls: Vec<CallSite>,                 // 呼び出し先
    try_catch_blocks: Vec<TryCatchBlock>, // 捕捉範囲
}

// 伝播計算結果
struct PropagatedThrow {
    error_type: ErrorType,
    origin: ThrowSite,      // 元のthrow位置
    path: Vec<FunctionId>,  // 伝播経路
}

// 最終レポート
struct Diagnostic {
    function: FunctionId,
    missing_throws: Vec<PropagatedThrow>,
}
```

## 処理フロー（2パス解析）

### Pass 1: インデックス構築

1. 各ファイルをoxcでパース
2. 関数定義を走査
   - JSDocから`@throws`抽出
   - 直接throw文を検出（フロー解析で型推論）
   - 関数呼び出しを検出
   - try-catchブロックの範囲を記録
3. `FunctionSignature`をMapに格納

### Pass 2: 伝播計算

1. call graphを構築（petgraph）
2. 各関数について:
   - 直接throwを収集
   - 呼び出し先のthrowを再帰的に収集
   - try-catchで捕捉済みのものを除外
   - re-throw（catch内throw）を追加
3. 宣言された`@throws`と突合
   - 型名の一致をチェック
   - 漏れをDiagnosticとして記録

### 出力

- `--format text`: 人間可読なエラー表示
- `--format json`: 構造化データ出力
- exit code: 漏れがあれば1、なければ0

## throw式のフロー解析

| パターン | 解析方法 |
|---------|---------|
| `throw new ValidationError()` | クラス名を直接抽出 |
| `const err = new X(); throw err;` | 変数の初期化式を解析 |
| `throw createError()` | 関数の実装を解析、または`Unknown` |
| `throw cond ? new A() : new B()` | 両ブランチを収集 |
| `throw "error"` | `Unknown`扱い |

**スコープ**: ローカル変数の単純代入のみ追跡。複雑な制御フローは`Unknown`にフォールバック。

## try-catchの捕捉判定

```typescript
try {
  validate();  // ValidationError
  save();      // DBError
} catch (e) {
  if (e instanceof ValidationError) {
    return;    // 捕捉済み → @throws不要
  }
  throw e;     // re-throw → @throws {DBError} 必要
}
```

- try-catchブロックの範囲を記録
- catch内の`instanceof`チェックを検出
- 捕捉された型を特定、re-throwされた型を追跡
- `instanceof`パターンのみ認識（安全側に倒す）

## 外部ライブラリの扱い

- デフォルトは`unknown`（追跡不可）
- 将来的に有名ライブラリ（fs/promises, zodなど）の情報をrepoにビルトイン追加
- ユーザーに設定ファイルを書かせる方式ではない

## async/awaitの扱い

**v1スコープ**:
- sync関数のthrow
- async関数内の直接throw
- try-catchでの捕捉

**v2で対応**:
- `Promise.reject()`
- `.catch()`チェーン
- Promise.allなどの完全追跡

## CLIインターフェース

```bash
# 基本使用（tsconfig.jsonから対象取得）
throw-trace check

# tsconfig指定
throw-trace check --project tsconfig.build.json

# ファイル/ディレクトリ直接指定
throw-trace check src/
throw-trace check src/service.ts
throw-trace check "src/**/*.ts"

# 出力形式
throw-trace check --format text   # デフォルト
throw-trace check --format json

# 除外パターン
throw-trace check --exclude "**/*.test.ts"
```

## 出力形式

### text形式

```
error: missing @throws declaration
  --> src/service.ts:15:3
   |
15 |   validate(input);
   |   ^^^^^^^^^^^^^^^ ValidationError propagates from src/validator.ts:8
   |
   = help: add @throws {ValidationError} to function createUser

Found 3 errors in 2 files
```

### json形式

```json
{
  "diagnostics": [
    {
      "file": "src/service.ts",
      "line": 15,
      "column": 3,
      "function": "createUser",
      "missing_throws": [
        {
          "error_type": "ValidationError",
          "origin_file": "src/validator.ts",
          "origin_line": 8
        }
      ]
    }
  ],
  "summary": {
    "errors": 3,
    "files_checked": 10
  }
}
```

## 依存クレート

```toml
[workspace.dependencies]
# パーサー
oxc_parser = "=0.73.0"
oxc_ast = "=0.73.0"
oxc_allocator = "=0.73.0"
oxc_span = "=0.73.0"

# call graph
petgraph = "=0.6.5"

# CLI
clap = { version = "=4.5.21", features = ["derive"] }

# 出力
serde = { version = "=1.0.215", features = ["derive"] }
serde_json = "=1.0.133"

# エラーハンドリング
thiserror = "=2.0.6"
anyhow = "=1.0.94"

# ファイル走査
ignore = "=0.4.23"
globset = "=0.4.15"
```

## テスト戦略

```
tests/
├── fixtures/           # テスト用TSファイル
│   ├── simple_throw.ts
│   ├── propagation.ts
│   ├── try_catch.ts
│   └── ...
└── integration/
    └── check_test.rs   # CLI統合テスト
```

**テストケース**:
- 直接throw検出
- 変数経由throw
- 関数間伝播
- try-catch捕捉
- re-throw
- async関数
- @throws型名一致/不一致
- 出力形式（text/json）

## v1スコープまとめ

| 機能 | v1 | v2 |
|------|----|----|
| 直接throw検出 | ✅ | |
| 変数経由throw | ✅ | |
| 関数間伝播 | ✅ | |
| try-catch捕捉 | ✅ | |
| re-throw追跡 | ✅ | |
| 型名厳密チェック | ✅ | |
| async内throw | ✅ | |
| Promise.reject | | ✅ |
| .catch()チェーン | | ✅ |
| 外部lib定義 | | ✅ |
| 設定ファイル | | ✅ |

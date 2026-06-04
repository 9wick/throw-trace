# 永続キャッシュ設計仕様書

`throw-trace` の大規模 codebase 向け差分解析を実現するため、`.throw-trace/cache.json` に永続キャッシュを保存する。

目的は、前回解析から変更されていないファイル・関数について、parse/extract、definition 解決、伝播計算、diagnostics 生成を可能な範囲で skip すること。正しさを優先し、依存関係や fingerprint を安全に判定できない場合は通常解析にフォールバックする。

## 背景・課題

- codebase が大きくなると、毎回すべての TypeScript ファイルを parse/extract し、tsserver で definition 解決し、全関数の伝播計算を行うコストが大きい
- `check` と `fix` は同じ `Analyzer` と diagnostics を使うべきであり、キャッシュも CLI サブコマンド別ではなく共通解析パイプラインに入れる
- function 単位で skip したいが、診断結果は関数本文だけで決まらない
- 呼び出し先、call graph、`@throws` 宣言、try/catch 範囲、型関係、tsconfig の変化も診断結果に影響する

## 設計方針

- キャッシュは `Analyzer` 内部で扱う
- `check` と `fix` は、今と同じく `Analyzer` から diagnostics を受け取る
- `.throw-trace/cache.json` を workspace local cache として使う
- キャッシュは2層に分ける
  - file extraction cache
  - function diagnostics cache
- cache hit の条件は保守的にする
- cache が壊れている、schema version が違う、hash が不足している場合は、そのエントリを無視する
- cache read/write 失敗で解析自体は失敗させない

## コンポーネント

### `cache.rs`

`.throw-trace/cache.json` の読み書きを担当する。

- cache file path の決定
- JSON load
- schema version 検証
- 壊れた JSON の無視
- atomic write
- cache directory 作成

### `CacheStore`

`Analyzer` が使う cache API。

- file extraction の lookup/update
- definition 解決結果の lookup/update
- diagnostics の lookup/update
- type check 結果の lookup/update

`Analyzer` は JSON の内部構造を直接触らず、`CacheStore` 経由で参照・更新する。

### `Fingerprint`

安定した hash を作る helper。

- 入力値を deterministic に並べる
- `HashMap` の iteration order に依存しない
- `PathBuf` は canonical path または安定化した absolute path として扱う
- `serde_json` 化する場合は key order を固定する

## キャッシュ形式

`.throw-trace/cache.json` は人間が読める JSON とする。

```json
{
  "schema_version": 1,
  "tool_version": "0.1.6",
  "workspace_fingerprint": "...",
  "files": {
    "/abs/path/src/a.ts": {
      "content_hash": "...",
      "extraction": {
        "signatures": [],
        "method_signatures": [],
        "type_relations": []
      }
    }
  },
  "definitions": {
    "...stable key...": {
      "caller_file": "/abs/path/src/a.ts",
      "caller_hash": "...",
      "callee_span": { "start": 10, "end": 14 },
      "callee_text": "foo",
      "definition_file": "/abs/path/src/b.ts",
      "definition_line": 3
    }
  },
  "diagnostics": {
    "...function key...": {
      "dependency_fingerprint": "...",
      "diagnostic": null
    }
  },
  "type_checks": {
    "...stable key...": true
  }
}
```

`diagnostic: null` は「この関数には missing throws がない」ことを表す。問題なし関数も cache hit 時に skip できる。

## Workspace Fingerprint

`workspace_fingerprint` は diagnostics cache の利用可否に使う。

含める情報:

- cache schema version
- throw-trace version
- 最寄りの `tsconfig.json` 内容 hash
- 解析対象 paths の正規化結果
- exclude patterns の正規化結果
- `AnalyzerConfig` のうち診断結果に影響する設定
  - `max_depth`
  - `max_files`
  - `cross_file`

`workspace_fingerprint` が変わった場合、diagnostics cache は使わない。file extraction cache はファイル内容 hash が一致すれば利用できる。

## File Extraction Cache

ファイル単位で `ExtractionResult` を保存する。

cache hit 条件:

- canonical file path が一致する
- current content hash が cached content hash と一致する
- cached extraction payload を復元できる

hit した場合、`extract_all(source, path)` を実行せず、保存済みの `FunctionSignature`、`MethodSignature`、`TypeRelation` を復元する。

miss した場合、通常通り `extract_all` を実行し、結果を cache に保存する。

## Definition Cache

tsserver の definition 解決結果を call site ごとに保存する。

cache key に含める情報:

- caller file canonical path
- caller file content hash
- callee span
- callee text

cache hit 条件:

- caller file content hash が一致する
- callee span が一致する
- 現在の source から切り出した callee text が cached callee text と一致する
- definition file path と definition line が復元できる

hit した場合、tsserver の `definition` 呼び出しを skip する。

miss した場合、既存通り tsserver に問い合わせ、成功した結果を cache に保存する。

## Function Diagnostics Cache

関数単位で diagnostics を保存する。

cache key は `FunctionId` 由来の安定 key とする。

- function file canonical path
- function name
- function span

cache value は以下を持つ。

- dependency fingerprint
- diagnostic または `null`

dependency fingerprint に含める情報:

- 関数自身の `FunctionSignature` hash
- 直接・推移的に呼ぶ関数の `FunctionSignature` hash
- caller/callee edge と call site location の hash
- 関連する `MethodSignature` hash
- 関連する `TypeRelation` hash
- workspace fingerprint
- type check cache key と結果の hash

cache hit 条件:

- function key が一致する
- dependency fingerprint が一致する
- diagnostic payload を復元できる

hit した場合、その関数の propagation と missing declaration check を skip する。

miss した場合、通常通り `compute_propagated_throws` と declaration check を実行し、結果を保存する。

## 処理フロー

1. `Analyzer::analyze_files` 開始時に `.throw-trace/cache.json` を読む
2. 各 TypeScript ファイルの content hash を計算する
3. file extraction cache が hit したファイルは `ExtractionResult` を復元する
4. miss したファイルだけ `extract_all` を実行する
5. 復元・新規抽出した全 signature を `Analyzer` に登録する
6. cross-file が有効な場合、definition cache を使いながら依存先ファイルを収集する
7. 新しく見つかった依存先ファイルも file extraction cache 経由で解析する
8. call graph を構築する
9. diagnostics 生成時に関数ごとの dependency fingerprint を計算する
10. diagnostics cache が hit した関数は cached result を使う
11. miss した関数だけ通常計算する
12. 解析終了時に cache を atomic に保存する

`fix` は diagnostics に基づいてソースを書き換える。書き換え後は content hash が変わるため、次回実行で対象ファイルの file extraction cache と関数 diagnostics cache は自然に invalidation される。

## エラー処理

- cache file が存在しない場合は empty cache として扱う
- JSON parse に失敗した場合は cache 全体を無視する
- schema version が違う場合は cache 全体を無視する
- 個別 entry の復元に失敗した場合は、その entry だけ無視する
- cache save に失敗しても diagnostics は返す
- definition cache の復元に失敗した場合は tsserver に問い合わせる
- diagnostics fingerprint が作れない場合は通常計算する

cache の都合で解析結果の正しさを落とさない。

## テスト方針

- 同じファイル内容なら file extraction cache が使われる
- ファイル内容が変わると file extraction cache が無効化される
- callee 側の関数 signature が変わると caller の diagnostics cache が無効化される
- call graph edge が変わると diagnostics cache が無効化される
- schema version が違う cache は安全に無視される
- 壊れた JSON でも通常解析にフォールバックする
- `check` と `fix` が同じ `Analyzer` cache を通る

## 非目標

- キャッシュ保存先の設定オプション追加
- OS 標準 cache directory への保存
- 外部ライブラリの throw 情報 database 化
- tsserver 自体のプロセス永続化
- perfect な incremental TypeScript compiler 互換

## 実装順序

1. cache data structure と load/save を追加する
2. file extraction cache を `Analyzer::analyze_file` に組み込む
3. definition cache を `collect_definition_targets` に組み込む
4. diagnostics cache のための dependency fingerprint を実装する
5. diagnostics generation に cache lookup/update を組み込む
6. cache invalidation と fallback のテストを追加する

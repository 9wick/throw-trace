# throw-trace

[![Crates.io](https://img.shields.io/crates/v/throw-trace)](https://crates.io/crates/throw-trace)
[![npm](https://img.shields.io/npm/v/throw-trace)](https://www.npmjs.com/package/throw-trace)

TypeScriptの`@throws` TSDoc宣言の漏れを検出する静的解析ツール。

関数が投げる可能性のある例外を追跡し、`@throws`が正しく宣言されているかをチェックします。

## インストール

```bash
# npm
npx throw-trace --help

# cargo
cargo install throw-trace
```

## 使い方

### 基本

```bash
# カレントディレクトリをチェック
throw-trace check

# 特定のファイルやディレクトリを指定
throw-trace check src/
throw-trace check src/service.ts
```

### オプション

```bash
# 除外パターンを指定
throw-trace check src/ --exclude "**/*.test.ts"

# JSON形式で出力
throw-trace check src/ --format json
```

## 検出例

以下のコードでは、`createUser`関数が`validate`を呼び出していますが、`@throws`が宣言されていません。

```typescript
/**
 * @throws {ValidationError} 入力が不正な場合
 */
function validate(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

// @throws宣言が漏れている
function createUser(name: string) {
  validate(name);  // ValidationErrorが伝播する可能性
  // ...
}
```

throw-traceはこれを検出し、以下のように報告します：

```
error: missing @throws declaration
  --> src/service.ts:createUser
   |
   | ValidationError propagates from src/validator.ts
   |
   = help: add @throws {ValidationError} to function createUser
```

## 対応パターン

### throw検出

```typescript
throw new ValidationError("msg");     // Named型として検出
throw new Error("msg");               // Named型として検出
throw "error";                        // Unknown型
```

### try-catch捕捉

```typescript
function safe() {
  try {
    riskyOperation();
  } catch (e) {
    if (e instanceof ValidationError) {
      return null;  // ValidationErrorは捕捉済み → @throws不要
    }
    throw e;        // その他はre-throw → @throws必要
  }
}
```

### 伝播追跡

呼び出し先の関数が投げる例外は、呼び出し元にも伝播します。throw-traceはcall graphを構築し、再帰的に追跡します。

## 出力形式

### text（デフォルト）

```
error: missing @throws declaration
  --> src/service.ts:createUser
   |
   | ValidationError propagates from Span { start: 50, end: 80 }
   |
   = help: add @throws {ValidationError} to function createUser

Found 3 errors in 2 files
```

### json

```json
{
  "diagnostics": [
    {
      "file": "src/service.ts",
      "function": "createUser",
      "missing_throws": [
        {
          "error_type": "ValidationError",
          "origin_file": "",
          "origin_line": 50
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

## 外部ライブラリについて

外部ライブラリ（npm packages、Node.js標準ライブラリ）の関数呼び出しは追跡対象外です。外部ライブラリを呼び出す境界となる関数には、手動で`@throws`を記述してください。

## ライセンス

MIT License - see [LICENSE](LICENSE) for details

// 既存 JSDoc との共存パターン（fix が正しいフォーマットで挿入するか）

class ParseError extends Error {}

// --- 既存 JSDoc なし → 新規 JSDoc ブロックを挿入 ---

/** @throws {ParseError} */
function noDoc() {
  throw new ParseError();
}

// --- 既存 JSDoc (description のみ) → @throws を追記 ---

/**
 * Parses the input.
 * @throws {ParseError}
 */
function withDescription() {
  throw new ParseError();
}

// --- 既存 JSDoc (@param あり) → @throws を追記 ---

/**
 * Parses the given input string.
 * @param input - The string to parse
 * @throws {ParseError}
 */
function withParam(input: string) {
  if (!input) throw new ParseError();
}

// --- 既に正しい @throws がある → 変更なし ---

/**
 * @throws {ParseError} When parsing fails
 */
function alreadyCorrect() {
  throw new ParseError();
}

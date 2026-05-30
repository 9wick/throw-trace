// 関数呼び出し経由の throw 伝播パターン

class DBError extends Error {}
class ParseError extends Error {}

/** @throws {DBError} */
function dbQuery() {
  throw new DBError();
}

/** @throws {ParseError} */
function parse(s: string) {
  throw new ParseError();
}

// --- 単純な伝播 → @throws 必要 ---

/**
 * @throws {DBError} from 02_propagation.ts:dbQuery
 */
function getUser(id: string) {
  return dbQuery();
}

// --- 2段伝播 → 元の throw 元が表示される ---

/**
 * @throws {DBError} from 02_propagation.ts:dbQuery
 */
function getUserName(id: string) {
  return getUser(id);
}

// --- 複数 callee → 複数 @throws ---

/**
 * @throws {DBError} from 02_propagation.ts:dbQuery
 * @throws {ParseError} from 02_propagation.ts:parse
 */
function loadAndParse(id: string) {
  const raw = dbQuery();
  return parse("data");
}

// --- callee に @throws がない関数 → 伝播なし ---

function helper() {
  return 42;
}

function callsHelper() {
  return helper();
}

// --- 既に @throws がある関数を呼ぶ → 内容一致なら変化なし ---

/**
 * @throws {DBError} Propagated
 */
function getItems() {
  return dbQuery();
}

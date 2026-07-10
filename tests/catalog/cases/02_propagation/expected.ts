// 関数呼び出し経由のthrow伝播同期

class DBError extends Error {}
class ParseError extends Error {}

/**
 * @throws {DBError} from before.ts:dbQuery
 */
function dbQuery() {
  throw new DBError();
}

/**
 * @throws {ParseError} from before.ts:parse
 */
function parse() {
  throw new ParseError();
}

/**
 * @throws {DBError} from before.ts:dbQuery
 */
function getUser() {
  return dbQuery();
}

/**
 * @throws {DBError} from before.ts:dbQuery
 */
function getUserName() {
  return getUser();
}

/**
 * @throws {DBError} from before.ts:dbQuery
 * @throws {ParseError} from before.ts:parse
 */
function loadAndParse() {
  dbQuery();
  return parse();
}

function helper() {
  return 42;
}

function callsHelper() {
  return helper();
}

/** @throws {DBError} Propagated by contract. */
function preservesDescribedDeclaration() {
  return dbQuery();
}

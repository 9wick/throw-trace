// 関数呼び出し経由のthrow伝播同期

class DBError extends Error {}
class ParseError extends Error {}

function dbQuery() {
  throw new DBError();
}

function parse() {
  throw new ParseError();
}

function getUser() {
  return dbQuery();
}

/** @throws {ParseError} from before.ts:parse */
function getUserName() {
  return getUser();
}

/**
 * @throws {DBError} from before.ts:dbQuery
 */
function loadAndParse() {
  dbQuery();
  return parse();
}

function helper() {
  return 42;
}

/** @throws {DBError} from before.ts:dbQuery */
function callsHelper() {
  return helper();
}

/** @throws {DBError} Propagated by contract. */
function preservesDescribedDeclaration() {
  return dbQuery();
}

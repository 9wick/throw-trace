// generic 型パラメータの throw パターン（constraint 置換）

class BaseError extends Error {}
class SubError extends BaseError {}

// --- 制約付き型パラメータ <E extends C> の throw → C の throw として扱う ---
// [KNOWN BUG] resolve_type (crates/throw-trace-ts/src/tsserver.rs) が quickinfo の
// 生テキスト "E extends BaseError" を thrown_type としてそのまま返すため、
// is_assignable_to が合成する `const _typeCheck: {declared} = null! as E extends BaseError;`
// が構文エラーとなり、宣言された型に関わらず必ず型エラー扱いになる。
// 期待: constraint (BaseError) への置換により、C の throw として妥当に扱われる。

/**
 * @throws {BaseError}
 */
function throwsConstrained<E extends BaseError>(err: E): void {
  throw err;
}

// --- caller にも constraint 置換後の型 (BaseError) を宣言 ---

/**
 * @throws {BaseError} from 43_generic_type_param.ts:throwsConstrained
 */
function callsConstrained(err: SubError) {
  throwsConstrained(err);
}

// --- 上位型 (Error) を宣言しても健全な over-approximation として成立する ---

/**
 * @throws {Error}
 */
function throwsConstrainedAsError<E extends BaseError>(err: E): void {
  throw err;
}

/**
 * @throws {Error} from 43_generic_type_param.ts:throwsConstrainedAsError
 */
function callsConstrainedAsError(err: SubError) {
  throwsConstrainedAsError(err);
}

// --- 制約なし <E> の throw ---
// TypeScript では extends 句のない型パラメータの暗黙の制約は unknown。
// constraint 置換の実装が同じ経路で unconstrained なケースも解決するなら、
// 既存の Unknown 扱い（@throws {unknown} で宣言可能）に収束するべき。

/**
 * @throws {unknown}
 */
function throwsUnconstrained<E>(err: E): void {
  throw err;
}

/**
 * @throws {unknown} from 43_generic_type_param.ts:throwsUnconstrained
 */
function callsUnconstrained(err: string) {
  throwsUnconstrained(err);
}

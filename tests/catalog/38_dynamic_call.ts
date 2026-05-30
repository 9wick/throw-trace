// 動的呼び出し・計算されたプロパティアクセスのパターン
//
// [STATUS: 未対応]
// 静的に callee を解決できないケースは追跡不可。

class DynError extends Error {}

/** @throws {DynError} */
function fnA() {
  throw new DynError();
}

function fnB() {
  return 1;
}

// --- 変数経由の呼び出し ---
// [FALSE NEGATIVE] 変数に入れた関数の throw は追跡されない

function variableCall(cond: boolean) {
  const fn = cond ? fnA : fnB;
  fn();
}

// --- computed property access ---
// [FALSE NEGATIVE] obj[key]() は追跡されない

function computedCall(obj: Record<string, () => void>, key: string) {
  obj[key]();
}

// --- destructured method ---
// 分割代入しても callee name ("find") でマッチするため伝播する。
// 注: 名前ベースマッチなので、同名の別関数と混同する可能性あり。

class Repo {
  /** @throws {DynError} */
  find(id: string): string {
    if (!id) throw new DynError();
    return id;
  }
}

/**
 * @throws {DynError} from 38_dynamic_call.ts:find
 */
function destructuredCall(repo: Repo) {
  const { find } = repo;
  find("x");
}

// --- optional chaining ---
// obj?.method() は callee として認識される（property name でマッチ）。

class MaybeService {
  /** @throws {DynError} */
  run(): void {
    throw new DynError();
  }
}

/**
 * @throws {DynError} from 38_dynamic_call.ts:run
 */
function optionalCall(svc?: MaybeService) {
  svc?.run();
}

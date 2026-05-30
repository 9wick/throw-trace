// Promise .catch() パターン
//
// [STATUS: 未対応]
// .catch() はメソッド呼び出しとして認識されるが、
// エラーハンドリング（reject の吸収）としては解釈されない。
// そのため .catch() で握り潰しても @throws を要求する false positive が発生する。

class NetworkError extends Error {}

/** @throws {NetworkError} */
async function fetchData() {
  throw new NetworkError();
}

// --- .catch() で握り潰し ---
// [FALSE POSITIVE] 期待: @throws 不要（.catch で処理済み）
// 現状: .catch がエラーハンドリングとして認識されない
/**
 * @throws {NetworkError} from 20_promise_catch.ts:fetchData
 */
function loadWithCatch() {
  return fetchData().catch(() => null);
}

// --- .catch() で選択的に処理 + rethrow ---
// [FALSE POSITIVE] 期待: @throws {NetworkError}（rethrow 分のみ）
// 現状: .catch の意味論が未解釈
/**
 * @throws {any} from 20_promise_catch.ts:selectiveCatch
 * @throws {NetworkError} from 20_promise_catch.ts:fetchData
 */
function selectiveCatch() {
  return fetchData().catch((e) => {
    if (e instanceof NetworkError) return null;
    throw e;
  });
}

// --- .catch() 内で別の error を throw ---
// [FALSE POSITIVE] 期待: @throws {Error}（.catch 内の throw のみ）
// 現状: 元の fetchData の throw も漏れる
/**
 * @throws {Error} from 20_promise_catch.ts:catchAndWrap
 * @throws {NetworkError} from 20_promise_catch.ts:fetchData
 */
function catchAndWrap() {
  return fetchData().catch(() => {
    throw new Error("wrapped");
  });
}

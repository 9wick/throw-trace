// .then().catch() チェーンの複合パターン
//
// [STATUS: 未対応]
// .catch() がチェーン全体の reject を吸収する意味論が未実装。

class HttpError extends Error {}

/** @throws {HttpError} */
async function httpGet() {
  throw new HttpError();
}

// --- .then().catch() で最後に catch ---
// [FALSE POSITIVE] 期待: @throws 不要（.catch で全吸収）
// 現状: .catch がエラーハンドリングとして認識されない
/**
 * @throws {HttpError} from 22_promise_then_catch_chain.ts:httpGet
 */
function thenThenCatch() {
  return httpGet()
    .then((x) => x)
    .then((x) => x)
    .catch(() => null);
}

// --- .catch().then() で catch 後に then ---
// [FALSE POSITIVE] 期待: @throws 不要（.catch が先に吸収）
// 現状: fetchData の throw がそのまま伝播
/**
 * @throws {HttpError} from 22_promise_then_catch_chain.ts:httpGet
 */
function catchThenThen() {
  return httpGet()
    .catch(() => "fallback")
    .then((x) => x);
}

// --- .then(onFulfilled, onRejected) の2引数形式 ---
// [FALSE POSITIVE] 期待: onRejected で処理 → @throws 不要
// 現状: 2引数形式も未対応
/**
 * @throws {HttpError} from 22_promise_then_catch_chain.ts:httpGet
 */
function thenTwoArgs() {
  return httpGet().then(
    (data) => data,
    () => null,
  );
}

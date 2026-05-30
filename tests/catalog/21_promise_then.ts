// Promise .then() コールバック内の throw パターン
//
// [STATUS: 部分的]
// .then() のコールバックは独立した関数として抽出され、
// その中の throw は検出される。ただし .then() チェーンを通じた
// reject フローの伝播意味論は未実装。

class ApiError extends Error {}
class TransformError extends Error {}

/** @throws {ApiError} */
async function callApi() {
  throw new ApiError();
}

// --- .then() コールバック内で throw ---
// [FALSE POSITIVE] .then 経由の callApi の throw がそのまま伝播扱い
// 期待: @throws {TransformError}（コールバック内の throw）
//       + @throws {ApiError}（callApi の reject）は .then 経由で伝播
// 現状: コールバックの throw は検出されるが、チェーン意味論なし
/**
 * @throws {TransformError} from 21_promise_then.ts:thenWithThrow
 * @throws {ApiError} from 21_promise_then.ts:callApi
 */
function thenWithThrow() {
  return callApi().then(() => {
    throw new TransformError();
  });
}

// --- .then() コールバックが throw しない場合 ---
// [FALSE POSITIVE] 期待: @throws {ApiError}（callApi の reject が素通り）
// 現状: 同じく伝播扱い（結果的には正しい報告だが理由が違う）
/**
 * @throws {ApiError} from 21_promise_then.ts:callApi
 */
function thenNoThrow() {
  return callApi().then((data) => data);
}

// --- .then().then() チェーン ---
// [FALSE POSITIVE] 期待: 各 .then の throw + 元の reject
// 現状: チェーン意味論なし
/**
 * @throws {ApiError} from 21_promise_then.ts:callApi
 */
function thenChain() {
  return callApi()
    .then((x) => x)
    .then((x) => x);
}

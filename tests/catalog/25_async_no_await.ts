// async 関数の戻り値を await せずに使うパターン
//
// [STATUS: 未対応]
// await なしで返された Promise の reject フローは追跡されない。
// ただし現状は関数呼び出し自体の throw として伝播するため、
// 結果的に正しい報告になるケースが多い。

class AsyncError extends Error {}

/** @throws {AsyncError} */
async function asyncOp() {
  throw new AsyncError();
}

// --- await なしで return ---
// 期待: @throws {AsyncError}（reject が caller に伝播）
// 現状: 呼び出しの throw として伝播（結果的に正しい）
/**
 * @throws {AsyncError} from 25_async_no_await.ts:asyncOp
 */
function returnWithoutAwait() {
  return asyncOp();
}

// --- 変数に入れて return ---
// 期待: @throws {AsyncError}
// 現状: 同上
/**
 * @throws {AsyncError} from 25_async_no_await.ts:asyncOp
 */
function assignAndReturn() {
  const p = asyncOp();
  return p;
}

// --- fire-and-forget（戻り値を使わない）---
// 期待: unhandled rejection の警告が本来欲しいが、スコープ外
// 現状: 呼び出しの throw として伝播
/**
 * @throws {AsyncError} from 25_async_no_await.ts:asyncOp
 */
function fireAndForget() {
  asyncOp();
}

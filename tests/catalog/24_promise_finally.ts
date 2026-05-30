// .finally() パターン
//
// [STATUS: 未対応]
// .finally() は reject を吸収しない（素通りさせる）が、
// 現在のツールはその意味論を理解していない。

class CleanupError extends Error {}
class OpError extends Error {}

/** @throws {OpError} */
async function operation() {
  throw new OpError();
}

// --- .finally() → reject は素通り ---
// 期待: @throws {OpError}（finally は reject を吸収しない）
// 現状: 結果的に正しい（operation の throw がそのまま伝播）
/**
 * @throws {OpError} from 24_promise_finally.ts:operation
 */
function withFinally() {
  return operation().finally(() => console.log("done"));
}

// --- .catch().finally() → catch で吸収 ---
// [FALSE POSITIVE] 期待: @throws 不要（.catch で吸収済み、.finally は関係ない）
// 現状: .catch の吸収が未対応
/**
 * @throws {OpError} from 24_promise_finally.ts:operation
 */
function catchThenFinally() {
  return operation()
    .catch(() => null)
    .finally(() => console.log("done"));
}

// --- .finally() 内で throw ---
// [FALSE POSITIVE] 期待: @throws {CleanupError}（finally 内の throw）
//                        + @throws {OpError}（operation の reject は素通り）
// 現状: finally 内の throw はコールバックとして検出される
/**
 * @throws {CleanupError} from 24_promise_finally.ts:throwInFinally
 * @throws {OpError} from 24_promise_finally.ts:operation
 */
function throwInFinally() {
  return operation().finally(() => {
    throw new CleanupError();
  });
}

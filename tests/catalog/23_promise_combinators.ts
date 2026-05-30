// Promise.all / Promise.race 等のコンビネータパターン
//
// [STATUS: 未対応]
// Promise コンビネータ経由の reject 伝播は未実装。
// 現状は引数の関数呼び出しの throw がそのまま伝播される。

class TaskError extends Error {}

/** @throws {TaskError} */
async function runTask() {
  throw new TaskError();
}

// --- Promise.all ---
// [FALSE POSITIVE] 期待: @throws {TaskError}（いずれかが reject したら reject）
// 現状: runTask の throw が直接伝播扱い（結果的に正しいが意味論が違う）
/**
 * @throws {TaskError} from 23_promise_combinators.ts:runTask
 */
function withPromiseAll() {
  return Promise.all([runTask(), runTask()]);
}

// --- Promise.race ---
// 同上
/**
 * @throws {TaskError} from 23_promise_combinators.ts:runTask
 */
function withPromiseRace() {
  return Promise.race([runTask()]);
}

// --- Promise.allSettled ---
// [FALSE POSITIVE] 期待: @throws 不要（allSettled は reject しない）
// 現状: runTask の throw がそのまま伝播
/**
 * @throws {TaskError} from 23_promise_combinators.ts:runTask
 */
function withPromiseAllSettled() {
  return Promise.allSettled([runTask()]);
}

// --- Promise.resolve / Promise.reject ---

function withPromiseResolve() {
  return Promise.resolve(42);
}

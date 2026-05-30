// コールバック内の throw パターン（Promise 以外）
//
// [STATUS: 部分的]
// コールバックは独立した関数として抽出されるが、
// 呼び出し元への伝播はコールバックの種類に依存する。

class ProcessError extends Error {}

/** @throws {ProcessError} */
function processItem(item: string) {
  if (!item) throw new ProcessError();
}

// --- Array.forEach 内で throw ---
// 期待: @throws {ProcessError}（forEach は同期的に throw を伝播する）
// 現状: forEach 内の processItem 呼び出しは検出される
/**
 * @throws {ProcessError} from 27_callback_throw.ts:processItem
 */
function forEachThrow(items: string[]) {
  items.forEach((item) => {
    processItem(item);
  });
}

// --- Array.map 内で throw ---
// 同上
/**
 * @throws {ProcessError} from 27_callback_throw.ts:processItem
 */
function mapThrow(items: string[]) {
  return items.map((item) => {
    processItem(item);
    return item;
  });
}

// --- コールバック内で直接 throw ---
/** @throws {ProcessError} */
function directThrowInCallback(items: string[]) {
  items.forEach(() => {
    throw new ProcessError();
  });
}

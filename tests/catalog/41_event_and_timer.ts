// setTimeout / setInterval / EventListener のパターン
//
// [STATUS: 部分的]
// コールバック内の throw は検出されるが、
// これらのコールバックは非同期に呼ばれるため、
// 実際には呼び出し元に throw が伝播しない。
// 現状は同期呼び出しと同じ扱いで伝播してしまう。

class TimerError extends Error {}

// --- setTimeout コールバック内で throw ---
// [FALSE POSITIVE] 期待: @throws 不要（setTimeout は非同期、throw は呼び出し元に届かない）
// 現状: コールバックの throw が伝播する
/**
 * @throws {TimerError} from 41_event_and_timer.ts:setTimeoutThrow
 */
function setTimeoutThrow() {
  setTimeout(() => {
    throw new TimerError();
  }, 0);
}

// --- setInterval ---
// [FALSE POSITIVE] 同上
/**
 * @throws {TimerError} from 41_event_and_timer.ts:setIntervalThrow
 */
function setIntervalThrow() {
  setInterval(() => {
    throw new TimerError();
  }, 1000);
}

// --- addEventListener ---
// [FALSE POSITIVE] 同上。イベントハンドラ内の throw は caller に届かない
/**
 * @throws {TimerError} from 41_event_and_timer.ts:addListenerThrow
 */
function addListenerThrow(el: EventTarget) {
  el.addEventListener("click", () => {
    throw new TimerError();
  });
}

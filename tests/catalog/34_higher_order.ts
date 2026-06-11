// 高階関数パターン（関数を返す / 関数を受け取る）

class HandlerError extends Error {}

// --- throw する関数を返す ---
// [FALSE POSITIVE] getHandler 自体は呼んでも throw しない（関数を返すだけ）。
// 現状: 無名関数に signature が作られず、内部の throw が getHandler に帰属される。
// 登録パターン（返した closure を呼ばず登録だけする）は 42_closure_registration.ts を参照

/**
 * @throws {HandlerError} from 34_higher_order.ts:getHandler
 */
function getHandler(): () => void {
  return () => {
    throw new HandlerError();
  };
}

// --- 返された関数を呼ぶ ---
// 現状: getHandler 経由の throw が伝播
/**
 * @throws {HandlerError} from 34_higher_order.ts:getHandler
 */
function callsReturnedFn() {
  const fn = getHandler();
  fn();
}

// --- 関数を引数として受け取って呼ぶ ---
// [FALSE NEGATIVE] 引数の関数が throw するかは静的に不明

function callsCallback(fn: () => void) {
  fn();
}

// --- 引数に throwing 関数を渡す ---
// [FALSE NEGATIVE] callsCallback を介した throw は追跡されない

/** @throws {HandlerError} */
function thrower() {
  throw new HandlerError();
}

function passesThrowingFn() {
  callsCallback(thrower);
}

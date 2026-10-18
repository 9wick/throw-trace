// 高階関数パターン（関数を返す / 関数を受け取る）

class HandlerError extends Error {}

// --- throw する関数を返す ---
// [STATUS: 対応] getHandler は関数を返すだけなので @throws 不要。
// 匿名関数は独立して解析し、実際の呼び出し経路へだけ例外を伝播する。
// 登録パターン（返した closure を呼ばず登録だけする）は 42_closure_registration.ts を参照

function getHandler(): () => void {
  return () => {
    throw new HandlerError();
  };
}

// --- 返された関数を呼ぶ ---
// [FALSE NEGATIVE] 返却値を変数へ代入して呼ぶ経路は未追跡。
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

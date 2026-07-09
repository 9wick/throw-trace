// decorator は class 定義時に実行される。
// method decorator factory の呼び出しは method 呼び出し時の throw ではなく、
// class 定義を囲むスコープ（トップレベルなら帰属先なし）に帰属されるべき。

class DecoratorError extends Error {}

/** @throws {DecoratorError} */
function methodTag(tag: string) {
  if (!tag) throw new DecoratorError();
  return function (value: Function, context: ClassMethodDecoratorContext) {};
}

// --- トップレベル class: decorator はモジュール評価時に実行 ---
// run() にも呼び出し側にも @throws は不要

class TopLevel {
  @methodTag("m")
  run() {
    return 1;
  }
}

export function callsRun(svc: TopLevel) {
  return svc.run();
}

// --- 関数内 class: decorator は囲む関数の実行時に走るため、
//     definesClass に @throws が要求されるのが正しい ---

export function definesClass() {
  class Inner {
    @methodTag("inner")
    innerRun() {
      return 1;
    }
  }
  return Inner;
}

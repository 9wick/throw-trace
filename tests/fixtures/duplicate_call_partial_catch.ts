// 同名関数の複数呼び出しで、一部だけ try-catch 内にあるケースの検証

class SomeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SomeError";
  }
}

/** @throws {SomeError} */
function risky(): void {
  throw new SomeError("boom");
}

// 1回目は try-catch 内、2回目は裸呼び出し
// → 2回目の呼び出しで SomeError が伝播するので @throws が必要
function callTwice(): void {
  try {
    risky();
  } catch (e) {
    return;
  }
  risky();
}

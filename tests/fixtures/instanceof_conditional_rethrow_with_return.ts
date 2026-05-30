// instanceof 分岐内で条件付き throw e + return があるケースの検証
// cond == true のパスで throw e（再送出）が到達可能

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

// if (cond) throw e; return; → cond=true で再送出される
// → SomeError は確実には捕捉されないので @throws が必要
function conditionalRethrow(cond: boolean): void {
  try {
    risky();
  } catch (e) {
    if (e instanceof SomeError) {
      if (cond) throw e;
      return;
    }
    throw e;
  }
}

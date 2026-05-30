// instanceof 分岐内の一部経路だけが終端するケースの検証

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

// instanceof ブロック内で条件付き return → cond が false なら fall-through → rethrow
// → SomeError は確実には捕捉されないので @throws が必要
function partialHandle(cond: boolean): void {
  try {
    risky();
  } catch (e) {
    if (e instanceof SomeError) {
      if (cond) return;
    }
    throw e;
  }
}

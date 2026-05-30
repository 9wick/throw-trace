// instanceof マッチ後に fall-through して rethrow されるケースの検証
// if-body が終端しない（return/throw なし）ので、SomeError は捕捉されない

class SomeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SomeError";
  }
}

/** @throws {SomeError} */
function riskyOp(): void {
  throw new SomeError("boom");
}

// instanceof で SomeError をチェックするが、ログを出すだけで return しない
// → fall-through して throw e で投げ直される → @throws {SomeError} が必要
function logAndRethrow(): void {
  try {
    riskyOp();
  } catch (e) {
    if (e instanceof SomeError) {
      console.log("caught SomeError");
    }
    throw e;
  }
}

// instanceof 分岐内で catch param を throw e する = 再送出であり捕捉ではない

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

// instanceof で SomeError をマッチした上で throw e → 再送出
// → SomeError は捕捉されていないので @throws が必要
function rethrowMatched(): void {
  try {
    risky();
  } catch (e) {
    if (e instanceof SomeError) {
      throw e;
    }
  }
}

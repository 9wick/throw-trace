// instanceof で特定エラーを捕捉 + 残りを rethrow した場合の検証

class TargetError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TargetError";
  }
}

/** @throws {TargetError} */
function riskyTarget(): void {
  throw new TargetError("boom");
}

// TargetError を instanceof で捕捉し、それ以外を rethrow
// → TargetError は捕捉済みなので @throws 不要のはず
function handleTarget(): number | null {
  try {
    riskyTarget();
  } catch (e) {
    if (e instanceof TargetError) {
      return null;
    }
    throw e;
  }
  return 1;
}

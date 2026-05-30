// if の全分岐が return で終端した後の到達不能な throw e を拾ってはいけない

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

// if/else 両方 return → throw e は到達不能 → SomeError は捕捉済み
// → @throws 不要
function handleWithUnreachable(cond: boolean): number | null {
  try {
    risky();
  } catch (e) {
    if (e instanceof SomeError) {
      if (cond) return 1;
      else return 2;
      throw e;
    }
    throw e;
  }
  return 0;
}

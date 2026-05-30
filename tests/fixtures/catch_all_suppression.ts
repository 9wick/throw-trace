// 素の catch (e) { ... } で全例外を握り潰した場合に @throws が不要になるかの検証

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

// catch-all で握り潰し → @throws 不要のはず
function safeCatchAll(): number | null {
  try {
    risky();
  } catch (e) {
    return null;
  }
  return 1;
}

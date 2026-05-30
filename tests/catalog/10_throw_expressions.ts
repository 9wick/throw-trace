// throw する式のバリエーション

class TypedError extends Error {}

// --- new Error() ---

/** @throws {TypedError} */
function throwsNew() {
  throw new TypedError();
}

// --- new Error("message") ---

/** @throws {TypedError} */
function throwsNewWithMessage() {
  throw new TypedError("something went wrong");
}

// --- throw 変数 ---

/** @throws {unknown} */
function throwsVariable() {
  const err = new TypedError();
  throw err;
}

// --- throw 条件式 ---

/** @throws {TypedError} */
function throwsConditional(x: boolean) {
  if (x) {
    throw new TypedError();
  }
}

// --- 複数の throw パス ---

/**
 * @throws {TypedError}
 * @throws {RangeError}
 */
function throwsMultiplePaths(x: number) {
  if (x < 0) throw new TypedError();
  if (x > 100) throw new RangeError();
}

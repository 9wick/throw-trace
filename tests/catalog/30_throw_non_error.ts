// Error 以外を throw するパターン

// --- string を throw ---

/** @throws {unknown} */
function throwString() {
  throw "something went wrong";
}

// --- number を throw ---

/** @throws {unknown} */
function throwNumber() {
  throw 42;
}

// --- null を throw ---

/** @throws {unknown} */
function throwNull() {
  throw null;
}

// --- undefined を throw ---

/** @throws {unknown} */
function throwUndefined() {
  throw undefined;
}

// --- object literal を throw ---

/** @throws {unknown} */
function throwObject() {
  throw { code: 500, message: "fail" };
}

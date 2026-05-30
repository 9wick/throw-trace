// switch/case 内の throw パターン

class InvalidStateError extends Error {}

// --- case 内で throw ---

/** @throws {InvalidStateError} */
function throwInCase(state: string) {
  switch (state) {
    case "error":
      throw new InvalidStateError();
    default:
      return;
  }
}

// --- default で throw (exhaustive check) ---

/** @throws {InvalidStateError} */
function exhaustiveSwitch(state: "a" | "b") {
  switch (state) {
    case "a":
      return 1;
    case "b":
      return 2;
    default:
      throw new InvalidStateError();
  }
}

// --- 全 case で throw ---

/** @throws {InvalidStateError} */
function allCasesThrow(state: string) {
  switch (state) {
    case "a":
      throw new InvalidStateError();
    case "b":
      throw new InvalidStateError();
    default:
      throw new InvalidStateError();
  }
}

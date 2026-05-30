// ネストした try-catch のパターン

class OuterError extends Error {}
class InnerError extends Error {}

/** @throws {OuterError} */
function outerRisky() {
  throw new OuterError();
}

/** @throws {InnerError} */
function innerRisky() {
  throw new InnerError();
}

// --- 内側の try で InnerError を捕捉、外側は素通り → OuterError のみ伝播 ---

/**
 * @throws {OuterError} from 09_nested_try_catch.ts:outerRisky
 */
function nestedCatch() {
  try {
    outerRisky();
    try {
      innerRisky();
    } catch {
      // InnerError caught here
    }
  } catch (e) {
    if (e instanceof InnerError) {
      return;
    }
    throw e;
  }
}

// --- 外側の catch-all で全捕捉 ---

function outerCatchAll() {
  try {
    try {
      innerRisky();
    } catch (e) {
      throw e;
    }
  } catch {
    return null;
  }
}

// --- 内側 rethrow + 外側 catch → 両方 caught ---

function innerRethrowOuterCatch() {
  try {
    try {
      innerRisky();
    } catch (e) {
      throw e;
    }
    outerRisky();
  } catch {
    return null;
  }
}

// finally ブロックのパターン

class CleanupError extends Error {}
class MainError extends Error {}

/** @throws {MainError} */
function mainOp() {
  throw new MainError();
}

// --- try-catch-finally: catch で捕捉 → @throws 不要 ---

function caughtWithFinally() {
  try {
    mainOp();
  } catch {
    return null;
  } finally {
    console.log("cleanup");
  }
}

// --- try-finally (catch なし) → throw が素通りする ---

/**
 * @throws {MainError} from 17_finally_block.ts:mainOp
 */
function tryFinallyNosCatch() {
  try {
    mainOp();
  } finally {
    console.log("cleanup");
  }
}

// --- finally 内で throw → finally 自体の throw ---

/** @throws {CleanupError} */
function throwInFinally() {
  try {
    return 1;
  } finally {
    throw new CleanupError();
  }
}

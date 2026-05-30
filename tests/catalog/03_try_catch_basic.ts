// try/catch による捕捉パターン（基本）

class AppError extends Error {}
class NetworkError extends Error {}

/** @throws {AppError} */
function risky() {
  throw new AppError();
}

/** @throws {NetworkError} */
function fetchData() {
  throw new NetworkError();
}

// --- catch-all（型ガードなし）→ 全捕捉 → @throws 不要 ---

function catchAll() {
  try {
    risky();
  } catch (e) {
    return null;
  }
}

function catchAllNoParam() {
  try {
    risky();
  } catch {
    return null;
  }
}

// --- 複数 callee を try 内で呼ぶ → 全部 caught ---

function catchAllMultiple() {
  try {
    risky();
    fetchData();
  } catch (e) {
    return null;
  }
}

// --- try の外で呼ぶ → uncaught ---

/**
 * @throws {NetworkError} from 03_try_catch_basic.ts:fetchData
 */
function outsideTry() {
  try {
    risky();
  } catch (e) {
    return null;
  }
  fetchData();
}

// --- rethrow (throw e) → 捕捉されない ---

/**
 * @throws {AppError} from 03_try_catch_basic.ts:risky
 */
function rethrowAll() {
  try {
    risky();
  } catch (e) {
    console.log(e);
    throw e;
  }
}

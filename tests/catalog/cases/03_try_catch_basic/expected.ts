// try/catchによる捕捉の同期基本ケース

class AppError extends Error {}
class NetworkError extends Error {}

/**
 * @throws {AppError} from before.ts:risky
 */
function risky() {
  throw new AppError();
}

/**
 * @throws {NetworkError} from before.ts:fetchData
 */
function fetchData() {
  throw new NetworkError();
}

function catchAll() {
  try {
    risky();
  } catch (error) {
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

function catchAllMultiple() {
  try {
    risky();
    fetchData();
  } catch (error) {
    return null;
  }
}

/**
 * @throws {NetworkError} from before.ts:fetchData
 */
function outsideTry() {
  try {
    risky();
  } catch (error) {
    return null;
  }
  fetchData();
}

/**
 * @throws {AppError} from before.ts:risky
 */
function rethrowAll() {
  try {
    risky();
  } catch (error) {
    throw error;
  }
}

// try/catchによる捕捉の同期基本ケース

class AppError extends Error {}
class NetworkError extends Error {}

function risky() {
  throw new AppError();
}

function fetchData() {
  throw new NetworkError();
}

/** @throws {AppError} from before.ts:risky */
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

/** @throws {NetworkError} from before.ts:fetchData */
function catchAllMultiple() {
  try {
    risky();
    fetchData();
  } catch (error) {
    return null;
  }
}

/** @throws {AppError} from before.ts:risky */
function outsideTry() {
  try {
    risky();
  } catch (error) {
    return null;
  }
  fetchData();
}

function rethrowAll() {
  try {
    risky();
  } catch (error) {
    throw error;
  }
}

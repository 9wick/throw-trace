// 直接throwの同期基本ケース

class AppError extends Error {}
class StaleError extends Error {}
class WrongError extends Error {}
class ManualError extends Error {}

/**
 * @throws {AppError} from before.ts:addsMissing
 */
function addsMissing() {
  throw new AppError();
}

/** @throws {AppError} */
function keepsCorrect() {
  throw new AppError();
}

function removesStale() {
  return 1;
}

/**
 * @throws {AppError} from before.ts:replacesWrong
 */
function replacesWrong() {
  throw new AppError();
}

/** @throws {ManualError} Raised by an external runtime hook. */
function preservesManualDescription() {
  return 2;
}

/** @throws {AppError} The operation failed. */
function preservesMatchingDescription() {
  throw new AppError();
}

function staysWithoutThrows() {
  return 3;
}

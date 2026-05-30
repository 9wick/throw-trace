// 直接 throw するパターンの基本ケース

class AppError extends Error {}
class NotFoundError extends Error {}

// --- throw あり → @throws 必要 ---

/** @throws {AppError} */
function throwsNew() {
  throw new AppError();
}

/** @throws {NotFoundError} */
function throwsConditionally(x: boolean) {
  if (x) throw new NotFoundError();
}

// --- throw なし → @throws 不要 ---

function noThrow() {
  return 1;
}

function returnsOnly(x: number): number {
  return x * 2;
}

// --- 既に正しい @throws がある → 変化なし ---

/**
 * @throws {AppError} When something fails
 */
function alreadyDocumented() {
  throw new AppError();
}

// --- 複数の throw → 複数の @throws ---

/**
 * @throws {AppError}
 * @throws {NotFoundError}
 */
function throwsMultiple(x: number) {
  if (x === 0) throw new AppError();
  if (x === 1) throw new NotFoundError();
}

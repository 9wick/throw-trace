// 静的メソッドのパターン

class AppError extends Error {
  /** @throws {AppError} */
  static create(msg: string): never {
    throw new AppError(msg);
  }
}

// --- 静的メソッド呼び出し → 伝播 ---

/**
 * @throws {AppError} from 19_static_method.ts:create
 */
function callStatic() {
  AppError.create("boom");
}

// --- 静的メソッドを try-catch で捕捉 ---

function callStaticCaught() {
  try {
    AppError.create("boom");
  } catch {
    return null;
  }
}

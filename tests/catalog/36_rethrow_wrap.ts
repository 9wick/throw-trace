// catch 内で新しいエラーを throw するパターン（ラップ・変換）

class OriginalError extends Error {}
class WrappedError extends Error {}
class DomainError extends Error {}

/** @throws {OriginalError} */
function lowLevel() {
  throw new OriginalError();
}

// --- catch 内で new Error を throw（ラップ）---
// 元の throw は catch-all で捕捉、新しい throw が発生

/** @throws {WrappedError} */
function wrapError() {
  try {
    lowLevel();
  } catch (e) {
    throw new WrappedError();
  }
}

// --- catch 内でドメインエラーに変換 ---

/** @throws {DomainError} */
function convertError() {
  try {
    lowLevel();
  } catch {
    throw new DomainError();
  }
}

// --- catch 内で条件付きラップ + rethrow ---
// instanceof で分岐、一方はラップ、他方は rethrow

/**
 * @throws {WrappedError}
 * @throws {OriginalError} from 36_rethrow_wrap.ts:lowLevel
 */
function conditionalWrap() {
  try {
    lowLevel();
  } catch (e) {
    if (e instanceof OriginalError) {
      throw new WrappedError();
    }
    throw e;
  }
}

// --- ラップしたエラーの伝播 ---

/**
 * @throws {WrappedError} from 36_rethrow_wrap.ts:wrapError
 */
function callsWrapper() {
  wrapError();
}

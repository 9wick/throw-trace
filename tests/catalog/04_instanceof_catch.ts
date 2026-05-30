// instanceof による型ガード付き catch パターン

class AuthError extends Error {}
class ValidationError extends Error {}

/** @throws {AuthError} */
function authenticate() {
  throw new AuthError();
}

/** @throws {ValidationError} */
function validate() {
  throw new ValidationError();
}

// --- instanceof + return で終端 → 捕捉済み ---

function caughtByInstanceof() {
  try {
    authenticate();
  } catch (e) {
    if (e instanceof AuthError) {
      return null;
    }
    throw e;
  }
}

// --- instanceof で fall-through（return なし）→ rethrow される → 未捕捉 ---

/**
 * @throws {AuthError} from 04_instanceof_catch.ts:authenticate
 */
function fallthroughRethrow() {
  try {
    authenticate();
  } catch (e) {
    if (e instanceof AuthError) {
      console.log("caught");
    }
    throw e;
  }
}

// --- instanceof ブロック内で throw e → 再送出であり捕捉ではない ---

/**
 * @throws {AuthError} from 04_instanceof_catch.ts:authenticate
 */
function rethrowInBranch() {
  try {
    authenticate();
  } catch (e) {
    if (e instanceof AuthError) {
      throw e;
    }
  }
}

// --- instanceof + 条件付き return → 一部パスで fall-through → 未捕捉 ---

/**
 * @throws {AuthError} from 04_instanceof_catch.ts:authenticate
 */
function partialTermination(cond: boolean) {
  try {
    authenticate();
  } catch (e) {
    if (e instanceof AuthError) {
      if (cond) return;
    }
    throw e;
  }
}

// --- if/else 全分岐 return 後の到達不能 throw e → 捕捉済み ---

function unreachableRethrow(cond: boolean) {
  try {
    authenticate();
  } catch (e) {
    if (e instanceof AuthError) {
      if (cond) return 1;
      else return 2;
      throw e;
    }
    throw e;
  }
}

// --- 条件付き throw e + return → rethrow パスあり → 未捕捉 ---

/**
 * @throws {AuthError} from 04_instanceof_catch.ts:authenticate
 */
function conditionalRethrow(cond: boolean) {
  try {
    authenticate();
  } catch (e) {
    if (e instanceof AuthError) {
      if (cond) throw e;
      return;
    }
    throw e;
  }
}

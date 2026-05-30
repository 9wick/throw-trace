// assertion 関数パターン
// asserts x is T の形の関数は失敗時に throw する

class AssertionError extends Error {}

/** @throws {AssertionError} */
function assertString(x: unknown): asserts x is string {
  if (typeof x !== "string") throw new AssertionError();
}

/** @throws {AssertionError} */
function assertDefined<T>(x: T | null | undefined): asserts x is T {
  if (x == null) throw new AssertionError();
}

// --- assertion 関数の呼び出し → 伝播 ---

/**
 * @throws {AssertionError} from 33_assertion_function.ts:assertString
 */
function usesAssert(val: unknown) {
  assertString(val);
  return val.toUpperCase();
}

// --- assertion 関数を try-catch で捕捉 ---

function safeAssert(val: unknown) {
  try {
    assertString(val);
    return val;
  } catch {
    return "default";
  }
}

// --- 複数の assertion 呼び出し ---

/**
 * @throws {AssertionError} from 33_assertion_function.ts:assertDefined
 * @throws {AssertionError} from 33_assertion_function.ts:assertString
 */
function multipleAssertions(val: unknown) {
  assertDefined(val);
  assertString(val);
  return val;
}

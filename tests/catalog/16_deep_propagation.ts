// 深い呼び出しチェーンの伝播

class DeepError extends Error {}

/** @throws {DeepError} */
function level0() {
  throw new DeepError();
}

/**
 * @throws {DeepError} from 16_deep_propagation.ts:level0
 */
function level1() {
  return level0();
}

/**
 * @throws {DeepError} from 16_deep_propagation.ts:level0
 */
function level2() {
  return level1();
}

/**
 * @throws {DeepError} from 16_deep_propagation.ts:level0
 */
function level3() {
  return level2();
}

// --- 途中で catch → 伝播が止まる ---

function level2Caught() {
  try {
    return level1();
  } catch {
    return null;
  }
}

function level3AfterCatch() {
  return level2Caught();
}

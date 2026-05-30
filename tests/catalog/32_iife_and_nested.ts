// IIFE・ネスト関数定義のパターン

class SetupError extends Error {}

/** @throws {SetupError} */
function riskySetup() {
  throw new SetupError();
}

// --- IIFE 内で throw ---
// IIFE の throw はコールバックとして検出・伝播される
/**
 * @throws {SetupError} from 32_iife_and_nested.ts:usesIIFE
 */
function usesIIFE() {
  (function () {
    throw new SetupError();
  })();
}

// --- IIFE 内で callee 呼び出し ---
/**
 * @throws {SetupError} from 32_iife_and_nested.ts:riskySetup
 */
function iifeCallsRisky() {
  (() => {
    riskySetup();
  })();
}

// --- ネストした関数定義 + 呼び出し ---
/**
 * @throws {SetupError} from 32_iife_and_nested.ts:inner
 */
function outerCallsInner() {
  /**
   * @throws {SetupError} from 32_iife_and_nested.ts:riskySetup
   */
  function inner() {
    riskySetup();
  }
  inner();
}

// --- ネストした関数を定義だけして呼ばない ---

function outerDefinesOnly() {
  /** @throws {SetupError} */
  function unused() {
    throw new SetupError();
  }
  return 1;
}

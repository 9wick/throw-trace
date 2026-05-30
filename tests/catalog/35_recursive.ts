// 再帰・相互再帰パターン

class RecursionError extends Error {}

// --- 単純再帰 ---

/**
 * @throws {RecursionError}
 */
function countdown(n: number): void {
  if (n <= 0) throw new RecursionError();
  countdown(n - 1);
}

// --- 相互再帰 ---

/**
 * @throws {RecursionError}
 */
function pingThrows(n: number): void {
  if (n <= 0) throw new RecursionError();
  pongThrows(n - 1);
}

/**
 * @throws {RecursionError} from 35_recursive.ts:pingThrows
 */
function pongThrows(n: number): void {
  pingThrows(n);
}

// --- 再帰呼び出し側 ---

/**
 * @throws {RecursionError} from 35_recursive.ts:countdown
 */
function usesRecursive() {
  countdown(10);
}

// --- 再帰 + try-catch ---

function safeRecursive() {
  try {
    countdown(10);
  } catch {
    return 0;
  }
}

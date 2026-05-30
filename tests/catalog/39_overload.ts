// オーバーロード・ジェネリック関数のパターン

class OverloadError extends Error {}

// --- オーバーロード関数 ---
// implementation signature の throw が検出される

function parse(x: number): number;
function parse(x: string): string;
/** @throws {OverloadError} */
function parse(x: number | string): number | string {
  if (typeof x === "number") throw new OverloadError();
  return x;
}

// --- 呼び出し側 ---

/**
 * @throws {OverloadError} from 39_overload.ts:parse
 */
function callsOverloaded() {
  return parse(42);
}

// --- ジェネリック関数内で throw ---

/** @throws {OverloadError} */
function assertExists<T>(val: T | null): T {
  if (val === null) throw new OverloadError();
  return val;
}

/**
 * @throws {OverloadError} from 39_overload.ts:assertExists
 */
function usesGeneric() {
  return assertExists<string>(null);
}

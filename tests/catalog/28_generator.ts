// generator 関数のパターン

class GenError extends Error {}

// --- generator 内で throw ---

/** @throws {GenError} */
function* syncGen() {
  yield 1;
  throw new GenError();
}

/** @throws {GenError} */
async function* asyncGen() {
  yield 1;
  throw new GenError();
}

// --- generator を呼ぶ側 ---
// generator() は iterator を返すだけで throw しない。
// throw が発生するのは .next() を呼んだとき。
// 現状: generator 呼び出し自体を throw 元として伝播する。
/**
 * @throws {GenError} from 28_generator.ts:syncGen
 */
function usesGenerator() {
  const it = syncGen();
  it.next();
}

// --- for...of で generator を消費 ---
/**
 * @throws {GenError} from 28_generator.ts:syncGen
 */
function iterateGenerator() {
  for (const x of syncGen()) {
    void x;
  }
}

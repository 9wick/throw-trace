// instanceof で複数の型を区別して catch するパターン

class ErrorA extends Error {}
class ErrorB extends Error {}
class ErrorC extends Error {}

/**
 * @throws {ErrorA}
 * @throws {ErrorB}
 * @throws {ErrorC}
 */
function riskyMulti(x: number) {
  if (x === 0) throw new ErrorA();
  if (x === 1) throw new ErrorB();
  throw new ErrorC();
}

// --- ErrorA だけ捕捉 → ErrorB, ErrorC が残る ---

/**
 * @throws {ErrorB} from 12_catch_instanceof_multiple_types.ts:riskyMulti
 * @throws {ErrorC} from 12_catch_instanceof_multiple_types.ts:riskyMulti
 */
function catchOnlyA() {
  try {
    riskyMulti(0);
  } catch (e) {
    if (e instanceof ErrorA) {
      return null;
    }
    throw e;
  }
}

// --- ErrorA と ErrorB を両方捕捉 → ErrorC だけ残る ---

/**
 * @throws {ErrorC} from 12_catch_instanceof_multiple_types.ts:riskyMulti
 */
function catchAandB() {
  try {
    riskyMulti(0);
  } catch (e) {
    if (e instanceof ErrorA) {
      return null;
    }
    if (e instanceof ErrorB) {
      return null;
    }
    throw e;
  }
}

// --- 全部捕捉 → @throws 不要 ---

function catchAll() {
  try {
    riskyMulti(0);
  } catch {
    return null;
  }
}

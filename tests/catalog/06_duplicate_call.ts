// 同名関数の複数呼び出しと部分的 try-catch

class IOError extends Error {}

/** @throws {IOError} */
function writeFile() {
  throw new IOError();
}

// --- 1回目 try 内、2回目 try 外 → 2回目が uncaught ---

/**
 * @throws {IOError} from 06_duplicate_call.ts:writeFile
 */
function writeWithPartialCatch() {
  try {
    writeFile();
  } catch {
    return;
  }
  writeFile();
}

// --- 両方 try 内 → 両方 caught ---

function writeBothCaught() {
  try {
    writeFile();
    writeFile();
  } catch {
    return;
  }
}

// --- 両方 try 外 → uncaught ---

/**
 * @throws {IOError} from 06_duplicate_call.ts:writeFile
 */
function writeBothUncaught() {
  writeFile();
  writeFile();
}

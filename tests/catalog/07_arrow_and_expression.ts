// アロー関数・関数式のパターン

class FormatError extends Error {}

/** @throws {FormatError} */
function formatData() {
  throw new FormatError();
}

// --- アロー関数内で throw ---

/** @throws {FormatError} */
const throwsInArrow = () => {
  throw new FormatError();
};

// --- アロー関数から callee 呼び出し ---

/**
 * @throws {FormatError} from 07_arrow_and_expression.ts:formatData
 */
const callsRisky = () => {
  return formatData();
};

// --- 関数式 ---

/** @throws {FormatError} */
const throwsInExpr = function() {
  throw new FormatError();
};

// --- アロー関数で catch → @throws 不要 ---

const safeArrow = () => {
  try {
    formatData();
  } catch {
    return null;
  }
};

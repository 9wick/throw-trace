// 制御フロー内の throw パターン（loop / early return / guard clause）

class GuardError extends Error {}

/** @throws {GuardError} */
function riskyOp() {
  throw new GuardError();
}

// --- for ループ内で throw ---

/** @throws {GuardError} */
function throwInForLoop(items: string[]) {
  for (const item of items) {
    if (!item) throw new GuardError();
  }
}

// --- while ループ内で throw ---

/** @throws {GuardError} */
function throwInWhile(n: number) {
  while (n > 0) {
    if (n === 3) throw new GuardError();
    n--;
  }
}

// --- guard clause（早期 return の前に throw）---

/** @throws {GuardError} */
function guardClause(input: string | null) {
  if (!input) throw new GuardError();
  return input.toUpperCase();
}

// --- if/else の両分岐で throw ---

/** @throws {GuardError} */
function throwBothBranches(cond: boolean) {
  if (cond) {
    throw new GuardError();
  } else {
    throw new GuardError();
  }
}

// --- ループ内で callee 呼び出し ---

/**
 * @throws {GuardError} from 37_control_flow.ts:riskyOp
 */
function callInLoop(n: number) {
  for (let i = 0; i < n; i++) {
    riskyOp();
  }
}

// --- ループ内で try-catch ---

function catchInLoop(n: number) {
  for (let i = 0; i < n; i++) {
    try {
      riskyOp();
    } catch {
      continue;
    }
  }
}

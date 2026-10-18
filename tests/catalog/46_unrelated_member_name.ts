// メソッド呼び出しを無関係な同名関数へ結び付けない

interface Scope { run<T>(fn: () => T): T }

export const runWithScope = (scope: Scope): void => scope.run(() => undefined);

export const makeTaskRunner = () => {
  /** @throws {RangeError} */
  const run = (): void => { throw new RangeError('unrelated run'); };
  return { run };
};

interface Scope { run<T>(fn: () => T): T }

export const runWithScope = (scope: Scope): void => scope.run(() => undefined);

export const makeTaskRunner = () => {
  /** @throws {RangeError} */
  const run = (): void => { throw new RangeError('unrelated run'); };
  return { run };
};

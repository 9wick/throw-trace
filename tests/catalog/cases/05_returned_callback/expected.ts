/** @throws {TypeError} */
const dangerous = (): void => { throw new TypeError('deferred'); };

export const makeDeferred = () =>
  /** @throws {TypeError} */
  () => dangerous();

export function makeFunction() {
  return /** @throws {TypeError} */ function () { dangerous(); };
}

export function makeNamedFunction() {
  return /** @throws {TypeError} */ function deferred() { dangerous(); };
}

/** @throws {TypeError} */
function dangerous() { throw new TypeError(); }

export const makeDeferred = () =>
  () => dangerous();

export function makeFunction() { return function () { dangerous(); }; }

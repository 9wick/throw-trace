/** @throws {TypeError} */
function dangerous() { throw new TypeError(); }

/** @throws {TypeError} from before.ts:dangerous */
export const makeDeferred = () =>
  () => dangerous();

export function makeFunction() { return function () { dangerous(); }; }

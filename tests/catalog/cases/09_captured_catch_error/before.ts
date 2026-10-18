/** @throws {TypeError} */
function dangerous() { throw new TypeError(); }

/** @throws {TypeError} */
export function getBody() {
  try { dangerous(); }
  catch (error) { return (() => { throw error; })(); }
}

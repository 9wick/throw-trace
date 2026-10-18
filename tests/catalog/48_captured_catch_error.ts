// 内側の関数からcatch変数を再送出するパターン

/** @throws {TypeError} */
function dangerous() { throw new TypeError(); }

/** @throws {TypeError} */
export function getBody() {
  try { dangerous(); }
  catch (error) { return (() => { throw error; })(); }
}

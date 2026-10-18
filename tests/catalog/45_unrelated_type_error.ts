// 無関係な型エラーがあっても上位型のthrows宣言を正しく認識する

/** @throws {TypeError} */
const dangerous = (): void => { throw new TypeError('example'); };

/** @throws {unknown} */
export const allowsUnknown = (): void => dangerous();

/** @throws {Error} */
export const allowsError = (): void => dangerous();

const unrelated: string = 123;

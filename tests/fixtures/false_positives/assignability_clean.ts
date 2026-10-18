/** @throws {TypeError} */
const dangerous = (): void => { throw new TypeError('example'); };

/** @throws {unknown} */
export const allowsUnknown = (): void => dangerous();

/** @throws {Error} */
export const allowsError = (): void => dangerous();

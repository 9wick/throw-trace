/** @throws {TypeError} */
const dangerous = (): void => { throw new TypeError('example'); };

/**
 * @throws {TypeError} from before.ts:dangerous
 */
export const allowsUnknown = (): void => dangerous();

/**
 * @throws {TypeError} from before.ts:dangerous
 */
export const allowsError = (): void => dangerous();

const unrelated: string = 123;

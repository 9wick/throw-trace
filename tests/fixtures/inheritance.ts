class BaseError extends Error {
  readonly _brand = 'BaseError';
}

class DerivedError extends BaseError {
  readonly _derived = true;
}

/** @throws {BaseError} */
export function catchesBase(): void {
  throw new DerivedError('test');
}

/** @throws {DerivedError} */
export function catchesDerived(): void {
  throw new BaseError('test');  // This should fail - BaseError is NOT assignable to DerivedError
}

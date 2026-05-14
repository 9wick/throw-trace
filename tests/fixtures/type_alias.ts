export class ErrorA extends Error {}
export class ErrorB extends Error {}

export type MyErrorUnion = ErrorA | ErrorB;

/** @throws {ErrorA | ErrorB} */
export function caller(): void {
  const err: MyErrorUnion = new ErrorA('test');
  throw err;
}

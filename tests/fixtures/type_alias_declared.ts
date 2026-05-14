import { ErrorA, MyErrorUnion } from './errors';

/** @throws {ErrorA} */
function inner(): void {
  throw new ErrorA('test');
}

/** @throws {MyErrorUnion} */
export function caller(): void {
  inner();
}

export class ErrorA extends Error {}
export class ErrorB extends Error {}

export type MyErrorUnion = ErrorA | ErrorB;

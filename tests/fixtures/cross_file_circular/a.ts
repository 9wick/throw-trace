import { processB } from "./b";

class ErrorA extends Error {
  name = "ErrorA" as const;
}

/**
 * @throws {ErrorA}
 */
export function processA(input: string): void {
  if (!input) {
    throw new ErrorA("Input required");
  }
  processB(input);
}

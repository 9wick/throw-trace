import { processA } from "./a";

class ErrorB extends Error {
  name = "ErrorB" as const;
}

/**
 * @throws {ErrorB}
 */
export function processB(input: string): void {
  if (input.length === 0) {
    throw new ErrorB("Empty input");
  }
  if (input === "recurse") {
    processA("nested");
  }
}

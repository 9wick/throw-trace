class ErrorA extends Error {
  name = "ErrorA" as const;
}

/**
 * @throws {ErrorA}
 */
export function validate(input: string): void {
  if (!input) {
    throw new ErrorA("Input required");
  }
}

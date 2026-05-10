// tests/fixtures/simple_throw.ts

// Missing @throws - should report error
function validate(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

/**
 * @throws {ValidationError} When input is invalid
 */
function validateWithDoc(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
  }
}

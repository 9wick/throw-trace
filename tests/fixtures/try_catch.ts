// tests/fixtures/try_catch.ts

/**
 * @throws {ValidationError} When validation fails
 */
function validate() {
  throw new ValidationError("Invalid");
}

// No @throws needed - ValidationError is caught
function safeValidate() {
  try {
    validate();
  } catch (e) {
    if (e instanceof ValidationError) {
      return null;
    }
    throw e;
  }
}

// Missing @throws {ValidationError} - not caught
function unsafeValidate() {
  try {
    validate();
  } catch (e) {
    console.log(e);
    throw e;
  }
}

class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
  }
}

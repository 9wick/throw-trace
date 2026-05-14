class KVError extends Error {
  constructor(public code: string, message: string) {
    super(message);
  }

  static invalidTtl(ttlSec: number): KVError {
    return new KVError('INVALID_TTL', `ttlSec must be > 0, got ${ttlSec}`);
  }

  static notFound(key: string): KVError {
    return new KVError('NOT_FOUND', `Key not found: ${key}`);
  }
}

/** @throws {KVError} */
function validateTtl(ttlSec: number | undefined): void {
  if (ttlSec !== undefined && ttlSec <= 0) throw KVError.invalidTtl(ttlSec);
}

/** @throws {KVError} */
function getRequired(key: string, value: string | undefined): string {
  if (value === undefined) throw KVError.notFound(key);
  return value;
}

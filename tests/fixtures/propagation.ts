// tests/fixtures/propagation.ts

/**
 * @throws {DBError} When database fails
 */
function dbQuery() {
  throw new DBError("Connection failed");
}

// Missing @throws {DBError} - should report error (propagation)
function getUser(id: string) {
  return dbQuery();
}

/**
 * @throws {DBError} When database fails
 */
function getUserWithDoc(id: string) {
  return dbQuery();
}

class DBError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DBError";
  }
}

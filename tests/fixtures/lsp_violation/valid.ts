interface Repository {
  /**
   * @throws {NotFoundError}
   */
  find(id: string): Entity;
}

class ImplementationA implements Repository {
  /**
   * @throws {NotFoundError}
   */
  find(id: string): Entity {
    throw new NotFoundError("not found");
  }
}

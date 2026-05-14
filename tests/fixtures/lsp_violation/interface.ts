interface UserRepository {
  /**
   * @throws {NotFoundError}
   */
  findById(id: string): User;

  save(user: User): void;
}

class DatabaseUserRepository implements UserRepository {
  findById(id: string): User {
    throw new DBError("connection failed");
  }

  save(user: User): void {
    throw new IOError("disk full");
  }
}

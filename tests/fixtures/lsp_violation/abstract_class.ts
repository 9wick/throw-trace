abstract class BaseRepository {
  /**
   * @throws {NotFoundError}
   */
  abstract findById(id: string): Entity;
}

class ConcreteRepository extends BaseRepository {
  findById(id: string): Entity {
    throw new ValidationError("invalid id");
  }
}

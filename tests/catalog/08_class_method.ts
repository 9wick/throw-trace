// クラスメソッド内の throw と伝播

class ServiceError extends Error {}
class RepoError extends Error {}

class Repository {
  /** @throws {RepoError} */
  load(id: string): string {
    if (!id) throw new RepoError();
    return id;
  }
}

class Service {
  private repo: Repository;

  constructor(repo: Repository) {
    this.repo = repo;
  }

  // method 内で直接 throw
  /** @throws {ServiceError} */
  validate(input: string): void {
    if (!input) throw new ServiceError();
  }

  // 別メソッド経由の伝播
  /**
   * @throws {RepoError} from 08_class_method.ts:load
   */
  getItem(id: string): string {
    return this.repo.load(id);
  }

  // try-catch で捕捉
  safeGetItem(id: string): string | null {
    try {
      return this.repo.load(id);
    } catch {
      return null;
    }
  }

  // 直接 throw + 伝播の両方
  /**
   * @throws {ServiceError}
   * @throws {RepoError} from 08_class_method.ts:load
   */
  process(id: string, input: string): string {
    this.validate(input);
    return this.repo.load(id);
  }
}

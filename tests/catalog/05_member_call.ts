// obj.method() 形式のメンバ呼び出しの伝播パターン

class NotFoundError extends Error {}
class WriteError extends Error {}

class UserRepo {
  /** @throws {NotFoundError} */
  find(id: string): string {
    if (!id) throw new NotFoundError();
    return id;
  }

  /** @throws {WriteError} */
  save(data: string): void {
    throw new WriteError();
  }

  list(): string[] {
    return [];
  }
}

// --- method 呼び出し → 伝播 ---

/**
 * @throws {NotFoundError} from 05_member_call.ts:find
 */
function getUser(repo: UserRepo, id: string) {
  return repo.find(id);
}

// --- throws しない method → 伝播なし ---

function listUsers(repo: UserRepo) {
  return repo.list();
}

// --- 複数 method → 複数 @throws ---

/**
 * @throws {NotFoundError} from 05_member_call.ts:find
 * @throws {WriteError} from 05_member_call.ts:save
 */
function updateUser(repo: UserRepo, id: string) {
  const user = repo.find(id);
  repo.save(user);
}

// --- method 呼び出しを try-catch で捕捉 → @throws 不要 ---

function safeFindUser(repo: UserRepo, id: string) {
  try {
    return repo.find(id);
  } catch {
    return null;
  }
}

// --- 同名メソッドが別クラスにある場合 ---

class OtherRepo {
  find(id: string): string {
    return id.toUpperCase();
  }
}

function noConflict(a: UserRepo, b: OtherRepo) {
  let result: string;
  try {
    result = a.find("x");
  } catch {
    result = "fallback";
  }
  return b.find(result);
}

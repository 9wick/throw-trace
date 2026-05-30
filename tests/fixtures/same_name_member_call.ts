// a.find() と b.find() は同名だが別クラスのメソッド。
// a.find() は throw する側で try-catch 内に配置。
// b.find() は throw しない側で try-catch 外に配置。
// 名前ベースマッチングだと b.find() の call site が a.find() 側にも拾われ、
// try-catch 外 → uncaught と誤判定される可能性がある。

class NotFoundError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "NotFoundError";
  }
}

class RepoA {
  /** @throws {NotFoundError} */
  find(id: string): string {
    if (!id) throw new NotFoundError("not found");
    return id;
  }
}

class RepoB {
  find(id: string): string {
    return id.toUpperCase();
  }
}

// a.find() は try-catch 内なので NotFoundError は caught。
// b.find() は throw しないので何も伝播しない。
// → 本来 "No issues found" になるべき。
function process(a: RepoA, b: RepoB): string {
  let result: string;
  try {
    result = a.find("x");
  } catch (e) {
    result = "fallback";
  }
  return b.find(result);
}

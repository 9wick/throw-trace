// obj.method() 形式のメンバ呼び出しで @throws が伝播するかの検証

class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
  }
}

class Validator {
  /** @throws {ValidationError} */
  check(x: string): void {
    if (!x) throw new ValidationError("required");
  }
}

// v.check(name) 経由で ValidationError が伝播するはず
function createUser(v: Validator, name: string): void {
  v.check(name);
}

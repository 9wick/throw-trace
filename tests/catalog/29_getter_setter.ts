// getter / setter 内の throw パターン

class AccessError extends Error {}

class Config {
  private _value: string | null = null;

  /** @throws {AccessError} */
  get value(): string {
    if (!this._value) throw new AccessError();
    return this._value;
  }

  /** @throws {AccessError} */
  set value(v: string) {
    if (!v) throw new AccessError();
    this._value = v;
  }
}

// --- getter アクセスによる伝播 ---
// 注: property access は現状 callee として認識されない。
// c.value は関数呼び出しではないため、伝播しない。
// [FALSE NEGATIVE] getter 経由の throw は追跡されない

function readConfig(c: Config) {
  return c.value;
}

// --- setter による伝播 ---
// [FALSE NEGATIVE] 同上

function writeConfig(c: Config, v: string) {
  c.value = v;
}

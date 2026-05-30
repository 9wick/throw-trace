// コンストラクタ内の throw パターン

class InitError extends Error {}

class Widget {
  /** @throws {InitError} */
  constructor(name: string) {
    if (!name) throw new InitError();
  }
}

// --- new Widget() の throw は現在伝播されない ---
// new 式は callee として認識されないため false negative。
// 将来 new 式対応後にここに @throws を追加すること。

function createWidget(name: string) {
  return new Widget(name);
}

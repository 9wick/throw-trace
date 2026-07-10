// decorated method への fix 挿入位置の検証用。
// fix は JSDoc を decorator の前に挿入し、挿入後の宣言が
// check / 再 fix で認識される（冪等になる）こと。

class BoomError extends Error {}

function tag(value: Function, context: ClassMethodDecoratorContext) {}

class Svc {
  @tag
  run() {
    throw new BoomError();
  }
}

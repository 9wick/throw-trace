// decorator 適用時の throw パターン（class / method / field decorator）
//
// decorator は class 定義時（モジュール評価時、または定義を囲む関数の実行時）に
// 実行される。constructor / method の呼び出し時ではない。

class DecoratorError extends Error {}

// --- 適用時に throw しうる decorator 関数（宣言済み）---

/** @throws {DecoratorError} */
function sealed(value: Function, context: ClassDecoratorContext) {
  throw new DecoratorError();
}

/** @throws {DecoratorError} */
function tagged(tag: string) {
  if (!tag) throw new DecoratorError();
  return function (value: Function, context: ClassDecoratorContext) {};
}

/** @throws {DecoratorError} */
function logMethod(value: Function, context: ClassMethodDecoratorContext) {
  if (!context) throw new DecoratorError();
}

/** @throws {DecoratorError} */
function methodTag(tag: string) {
  if (!tag) throw new DecoratorError();
  return function (value: Function, context: ClassMethodDecoratorContext) {};
}

/** @throws {DecoratorError} */
function fieldTag(tag: string) {
  if (!tag) throw new DecoratorError();
  return function (value: undefined, context: ClassFieldDecoratorContext) {};
}

// --- トップレベルの class decorator（bare identifier）---
// @sealed は CallExpression ではないため適用自体は追跡されない。
// throw はモジュール評価時に飛ぶもので、constructor 呼び出し側に
// 宣言を要求しないのは正しい挙動。

@sealed
class SealedService {
  constructor() {}
}

function createSealedService() {
  return new SealedService();
}

// --- トップレベルの class decorator factory 呼び出し ---
// tagged("service") はモジュール評価時の呼び出しで、囲む関数が
// 存在しないため帰属先なし。@throws は要求されない。

@tagged("service")
class TaggedService {}

// --- method decorator（bare identifier）---
// CallExpression ではないため未追跡。greet() の呼び出し側にも伝播しない。

class WithMethodDecorator {
  @logMethod
  greet() {
    return "hi";
  }
}

function callsDecoratedMethod(svc: WithMethodDecorator) {
  return svc.greet();
}

// --- method decorator factory 呼び出し ---
// [FALSE POSITIVE] methodTag("m") は class 定義時に実行されるが、
// decorator が MethodDefinition ノードに属し method の関数スコープ内で
// 走査されるため、method 呼び出し時の throw として誤帰属される。
// さらに member call 伝播で呼び出し側にも連鎖する。
// 期待: run() にも caller にも @throws 不要。
// [KNOWN BUG] fix は JSDoc を decorator と method 名の間に挿入するが、
// その位置の JSDoc は宣言として認識されず fix が冪等にならない
// （実行のたびに @throws 行が重複追記される）。
// ここでは認識される位置（decorator の前）に配置している。

class WithMethodFactory {
  /**
   * @throws {DecoratorError} from 44_decorator.ts:methodTag
   */
  @methodTag("m")
  run() {
    return 1;
  }
}

/**
 * @throws {DecoratorError} from 44_decorator.ts:methodTag
 */
function callsFactoryDecoratedMethod(svc: WithMethodFactory) {
  return svc.run();
}

// --- field decorator factory 呼び出し ---
// field 初期化子は関数スコープ外のため追跡されない（method factory と非一貫）。

class WithFieldFactory {
  @fieldTag("f")
  field = 1;
}

// --- 関数内での decorator 適用 ---
// class 定義が関数内にある場合、decorator は関数の実行時に走るため、
// 囲む関数への帰属は正当（この関数を呼ぶと decorator の throw が飛びうる）。

/**
 * @throws {DecoratorError} from 44_decorator.ts:tagged
 */
function createFallbackClass() {
  @tagged("fallback")
  class Fallback {}
  return Fallback;
}

// --- 参考: legacy parameter decorator（experimentalDecorators）---
// NestJS 等の constructor(@Inject("x") dep) 形式では、decorator factory の
// 呼び出しが constructor のパラメータリスト内で走査されるため、method factory
// と同様に constructor へ誤帰属される [FALSE POSITIVE]。
// TC39 decorator ではパラメータ decorator が存在せず型エラーになるため、
// このカタログには含めない。

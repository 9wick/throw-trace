// デフォルト引数・クラスフィールド初期化子のパターン
//
// [STATUS: 部分的]
// デフォルト引数内の関数呼び出しの throw は追跡される場合がある。
// クラスフィールド初期化子は関数スコープ外のため通常は追跡されない。

class InitError extends Error {}

/** @throws {InitError} */
function createDefault(): string {
  throw new InitError();
}

// --- デフォルト引数で throwing 関数を呼ぶ ---
// 現状: createDefault の throw が伝播する
/**
 * @throws {InitError} from 40_default_param.ts:createDefault
 */
function withDefault(x: string = createDefault()) {
  return x;
}

// --- デフォルト引数で直接 throw は構文エラー（TS で不可） ---
// function invalid(x: string = throw new E()) {} // SyntaxError

// --- クラスフィールド初期化子 ---
// [FALSE NEGATIVE] フィールド初期化子内の throw は constructor 経由で
// 伝播するが、new 式が callee として認識されないため追跡されない

class WithFieldInit {
  /**
   * @throws {InitError} from 40_default_param.ts:createDefault
   */
  value = createDefault();
}

// closure 登録パターン（コールバック登録 / イベントハンドラ）

declare function onError(h: (err: Error) => Promise<Response>): void;

// --- throw する closure を返すファクトリ ---
// [FALSE POSITIVE] createHandler 自体は呼んでも throw しない（closure を返すだけ）。
// 現状: 無名 closure には signature が作られず、closure 内の throw が
// 囲んでいる createHandler に帰属される。closure 直前に書いた JSDoc @throws も無視される。
// 注: fix は JSDoc を `export const createHandler =` の後（arrow chain の途中）に挿入する

/**
 * @throws {Error} from 42_closure_registration.ts:createHandler
 */
export const createHandler =
  (flag: () => boolean) =>
  async (err: Error): Promise<Response> => {
    if (!flag()) throw err;
    return new Response('ok');
  };

// --- closure を登録するだけで呼ばない ---
// [FALSE POSITIVE] registerHandler はいかなる経路でも throw しない。
// closure を呼ぶのはフレームワーク側だが、ファクトリに帰属された throw が
// ファクトリ呼び出し元の registerHandler へ伝播してしまう

/**
 * @throws {Error} from 42_closure_registration.ts:createHandler
 */
export const registerHandler = (): void => {
  onError(createHandler(() => true));
};

// --- 型注釈付きパラメータの rethrow ---
// 現状: 同一ファイル内なら tsserver の型解決で Error と解決される。
// クロスファイルの同パターンは tests/fixtures/cross_file_rethrow を参照

/**
 * @throws {Error} from 42_closure_registration.ts:rethrowParam
 */
export const rethrowParam = (err: Error): void => {
  throw err;
};

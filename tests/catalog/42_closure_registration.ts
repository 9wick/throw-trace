// closure 登録パターン（コールバック登録 / イベントハンドラ）

declare function onError(h: (err: Error) => Promise<Response>): void;

// --- throw する closure を返すファクトリ ---
// [STATUS: 対応] createHandler は closure を返すだけなので @throws 不要。
// 匿名関数は伝播解析に使うが、単独の @throws は必須にしない。

export const createHandler =
  (flag: () => boolean) =>
  async (err: Error): Promise<Response> => {
    if (!flag()) throw err;
    return new Response('ok');
  };

// --- closure を登録するだけで呼ばない ---
// registerHandler は closure を作成・登録するだけなので @throws 不要。
// ファクトリの呼び出し元へ closure の throw を伝播させない。

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

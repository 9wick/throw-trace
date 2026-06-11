// 型注釈付きパラメータの rethrow をクロスファイルで参照されるモジュール

/**
 * @throws {Error}
 */
export function rethrowParam(err: Error): void {
  throw err;
}

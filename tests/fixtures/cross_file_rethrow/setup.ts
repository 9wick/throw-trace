// クロスファイルでも throw 元（factory.ts）の文脈で型解決され、
// 上位型 Error の宣言で満たせることを検証する

import { rethrowParam } from "./factory";

/**
 * @throws {Error}
 */
export function setup(err: Error): void {
  rethrowParam(err);
}

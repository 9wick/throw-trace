// 構造型とnullを含むthrows宣言の判定

/** @throws {{} | null} */
export function runBuild(value: {} | null) { throw value; }

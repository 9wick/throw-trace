// new Promise() コンストラクタ内の reject/throw パターン
//
// [STATUS: 未対応]
// Promise コンストラクタの executor 内の throw や reject() は追跡されない。

class CustomError extends Error {}

// --- executor 内で throw ---
// executor 内の throw は Promise が rejection に変換する。
// 外側の同期 throw としては伝播しない。匿名 executor 自身への宣言も必須にしない。
function promiseWithThrow() {
  return new Promise((resolve) => {
    throw new CustomError();
  });
}

// --- executor 内で reject() ---
// [FALSE NEGATIVE] 期待: reject の型を追跡したいが、reject() の引数型は不明
// 現状: reject() は追跡対象外
function promiseWithReject() {
  return new Promise((resolve, reject) => {
    reject(new CustomError());
  });
}

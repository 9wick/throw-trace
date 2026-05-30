// new Promise() コンストラクタ内の reject/throw パターン
//
// [STATUS: 未対応]
// Promise コンストラクタの executor 内の throw や reject() は追跡されない。

class CustomError extends Error {}

// --- executor 内で throw ---
// executor 内の throw はコールバックの throw として検出・伝播される。
// 注: 実際の runtime では Promise が reject に変換するため throw としては飛ばないが、
//     現状は sync throw と同じ扱いで伝播する（結果的に正しい報告）。
/**
 * @throws {CustomError} from 26_promise_constructor.ts:promiseWithThrow
 */
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

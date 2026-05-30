// export された関数のパターン

class ApiError extends Error {}

// --- export function ---

/** @throws {ApiError} */
export function exportedThrow() {
  throw new ApiError();
}

// --- export const arrow ---

/** @throws {ApiError} */
export const exportedArrow = () => {
  throw new ApiError();
};

// --- export function with no throw ---

export function exportedSafe() {
  return 42;
}

// NOTE: default export function は現在のツールで自身の throw を
// 再伝播してしまうバグがある（自身を callee として認識してしまう）。
// 修正後に以下をアンコメントして catalog に追加すること:
//
// /** @throws {ApiError} */
// export default function defaultExported() {
//   throw new ApiError();
// }

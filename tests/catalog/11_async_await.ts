// async/await パターン
// 現在のツールは sync/async を区別せず throw を追跡するため、
// await で呼ぶ限りは正しく伝播する

class FetchError extends Error {}
class TimeoutError extends Error {}

/** @throws {FetchError} */
async function fetchRemote() {
  throw new FetchError();
}

/** @throws {TimeoutError} */
async function withTimeout() {
  throw new TimeoutError();
}

// --- await で呼ぶ → 伝播する ---

/**
 * @throws {FetchError} from 11_async_await.ts:fetchRemote
 */
async function getData() {
  return await fetchRemote();
}

// --- try-catch + await → 捕捉される ---

async function safeGetData() {
  try {
    return await fetchRemote();
  } catch {
    return null;
  }
}

// --- async 関数内で直接 throw ---

/** @throws {FetchError} */
async function asyncDirectThrow() {
  throw new FetchError();
}

// --- 複数 await ---

/**
 * @throws {FetchError} from 11_async_await.ts:fetchRemote
 * @throws {TimeoutError} from 11_async_await.ts:withTimeout
 */
async function multipleAwaits() {
  const data = await fetchRemote();
  await withTimeout();
}

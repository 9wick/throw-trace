// 同一関数内で caught と uncaught が混在するパターン

class DbError extends Error {}
class CacheError extends Error {}
class ConfigError extends Error {}

/** @throws {DbError} */
function queryDb() {
  throw new DbError();
}

/** @throws {CacheError} */
function readCache() {
  throw new CacheError();
}

/** @throws {ConfigError} */
function loadConfig() {
  throw new ConfigError();
}

// --- queryDb は try 内、readCache は try 外 → readCache だけ伝播 ---

/**
 * @throws {CacheError} from 14_mixed_caught_uncaught.ts:readCache
 */
function mixedCalls() {
  try {
    queryDb();
  } catch {
    // caught
  }
  readCache();
}

// --- 直接 throw + 捕捉された伝播 → 直接 throw だけ残る ---

/** @throws {ConfigError} */
function directThrowPlusCaughtPropagation() {
  try {
    queryDb();
  } catch {
    // caught
  }
  throw new ConfigError();
}

// --- 全部 try 外 → 全部伝播 ---

/**
 * @throws {DbError} from 14_mixed_caught_uncaught.ts:queryDb
 * @throws {CacheError} from 14_mixed_caught_uncaught.ts:readCache
 * @throws {ConfigError} from 14_mixed_caught_uncaught.ts:loadConfig
 */
function allUncaught() {
  queryDb();
  readCache();
  loadConfig();
}

// --- 全部 try 内 → 全部 caught ---

function allCaught() {
  try {
    queryDb();
    readCache();
    loadConfig();
  } catch {
    return null;
  }
}

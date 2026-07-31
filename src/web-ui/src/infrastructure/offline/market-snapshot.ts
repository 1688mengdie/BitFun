/**
 * 行情快照离线存储
 *
 * 将拉取的 K 线/行情数据缓存到 IndexedDB，
 * 支持离线时读取缓存快照，上线后自动更新。
 *
 * @module infrastructure/offline/market-snapshot
 */

import { getOfflineDb, type MarketSnapshotRecord } from './db';

/**
 * 保存行情快照（upsert）
 */
export async function saveMarketSnapshot(
  symbol: string,
  timeframe: string,
  data: unknown,
): Promise<void> {
  const db = await getOfflineDb();
  const id = `${symbol}-${timeframe}`;
  const now = Date.now();

  const existing = await db.get('market-snapshots', id);
  const version = existing ? existing.version + 1 : 1;

  await db.put('market-snapshots', {
    id,
    symbol,
    timeframe,
    data,
    timestamp: now,
    version,
  });
}

/**
 * 批量保存行情快照
 */
export async function saveMarketSnapshots(
  snapshots: Array<{
    symbol: string;
    timeframe: string;
    data: unknown;
  }>,
): Promise<void> {
  const db = await getOfflineDb();
  const tx = db.transaction('market-snapshots', 'readwrite');
  const now = Date.now();

  for (const snap of snapshots) {
    const id = `${snap.symbol}-${snap.timeframe}`;
    const existing = await tx.store.get(id);
    const version = existing ? existing.version + 1 : 1;

    tx.store.put({
      id,
      symbol: snap.symbol,
      timeframe: snap.timeframe,
      data: snap.data,
      timestamp: now,
      version,
    });
  }

  await tx.done;
}

/**
 * 读取单个行情快照
 */
export async function getMarketSnapshot(
  symbol: string,
  timeframe: string,
): Promise<MarketSnapshotRecord | undefined> {
  const db = await getOfflineDb();
  const id = `${symbol}-${timeframe}`;
  return db.get('market-snapshots', id);
}

/**
 * 读取某个合约的所有快照
 */
export async function getMarketSnapshotsBySymbol(
  symbol: string,
): Promise<MarketSnapshotRecord[]> {
  const db = await getOfflineDb();
  return db.getAllFromIndex('market-snapshots', 'symbol', symbol);
}

/**
 * 清理超过指定时间的旧快照
 */
export async function pruneOldSnapshots(
  maxAgeMs: number = 7 * 24 * 60 * 60 * 1000, // 默认 7 天
): Promise<number> {
  const db = await getOfflineDb();
  const cutoff = Date.now() - maxAgeMs;
  const allSnapshots = await db.getAll('market-snapshots');
  let pruned = 0;

  const tx = db.transaction('market-snapshots', 'readwrite');
  for (const snap of allSnapshots) {
    if (snap.timestamp < cutoff) {
      await tx.store.delete(snap.id);
      pruned++;
    }
  }
  await tx.done;

  return pruned;
}

/**
 * 获取所有已缓存的合约代码列表
 */
export async function getCachedSymbols(): Promise<string[]> {
  const db = await getOfflineDb();
  const all = await db.getAll('market-snapshots');
  const symbols = new Set(all.map((s) => s.symbol));
  return Array.from(symbols).sort();
}

/**
 * 离线 IndexedDB 数据库定义
 *
 * 架构总纲 §4a — 离线层（PWA）
 * 技术选型：idb (IndexedDB wrapper)
 *
 * Stores:
 *   - market-snapshots: 行情快照离线缓存
 *   - operation-queue:  操作队列（断线时暂存，恢复后重放）
 *   - cache-metadata:   缓存元数据（版本、时间戳等）
 *
 * @module infrastructure/offline/db
 */

import { openDB, IDBPDatabase } from 'idb';

const DB_NAME = 'bitfun-offline';
const DB_VERSION = 1;

export interface MarketSnapshotRecord {
  id: string;                    // `${symbol}-${timeframe}`
  symbol: string;                // 合约代码，如 "rb2510"
  timeframe: string;             // 周期，如 "1m", "5m", "1d"
  data: unknown;                 // 快照数据（序列化 JSON）
  timestamp: number;             // 更新时的 unix ms
  version: number;               // 版本号
}

export interface OperationQueueRecord {
  id: string;                    // UUID v7
  type: string;                  // 操作类型，如 "place_order", "cancel_order"
  payload: unknown;              // 操作数据
  createdAt: number;             // 创建时间 unix ms
  status: 'pending' | 'processing' | 'completed' | 'failed';
  retryCount: number;            // 重试次数
  lastError?: string;            // 最后错误信息
}

export interface CacheMetadataRecord {
  key: string;                   // 元数据键
  value: string;                 // 元数据值
  updatedAt: number;             // 更新时间
}

export type OfflineDb = IDBPDatabase<{
  'market-snapshots': MarketSnapshotRecord;
  'operation-queue': OperationQueueRecord;
  'cache-metadata': CacheMetadataRecord;
}>;

let dbInstance: OfflineDb | null = null;

/**
 * 获取离线数据库实例（懒初始化 + 单例）
 */
export async function getOfflineDb(): Promise<OfflineDb> {
  if (dbInstance) return dbInstance;

  dbInstance = (await openDB(DB_NAME, DB_VERSION, {
    upgrade(db, _oldVersion, _newVersion, _transaction) {
      // ---- market-snapshots ----
      if (!db.objectStoreNames.contains('market-snapshots')) {
        const snapshotStore = db.createObjectStore('market-snapshots', {
          keyPath: 'id',
        });
        snapshotStore.createIndex('symbol', 'symbol', { unique: false });
        snapshotStore.createIndex('timestamp', 'timestamp', { unique: false });
        snapshotStore.createIndex(
          'symbol_timeframe',
          ['symbol', 'timeframe'],
          { unique: false },
        );
      }

      // ---- operation-queue ----
      if (!db.objectStoreNames.contains('operation-queue')) {
        const queueStore = db.createObjectStore('operation-queue', {
          keyPath: 'id',
        });
        queueStore.createIndex('status', 'status', { unique: false });
        queueStore.createIndex('createdAt', 'createdAt', { unique: false });
        queueStore.createIndex('status_created', ['status', 'createdAt'], {
          unique: false,
        });
      }

      // ---- cache-metadata ----
      if (!db.objectStoreNames.contains('cache-metadata')) {
        db.createObjectStore('cache-metadata', {
          keyPath: 'key',
        });
      }
    },
  })) as unknown as OfflineDb;

  return dbInstance;
}

/**
 * 关闭数据库连接（主要用于测试）
 */
export async function closeOfflineDb(): Promise<void> {
  if (dbInstance) {
    dbInstance.close();
    dbInstance = null;
  }
}

/**
 * 删除整个离线数据库（主要用于测试/重置）
 */
export async function deleteOfflineDb(): Promise<void> {
  await closeOfflineDb();
  // indexedDB.deleteDatabase requires type assertion for the callback API
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.deleteDatabase(DB_NAME);
    request.addEventListener('success', () => resolve());
    request.addEventListener('error', () =>
      reject(new Error(`Failed to delete database ${DB_NAME}`)),
    );
  });
}

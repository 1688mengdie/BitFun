/**
 * 离线层（PWA）— 遁符
 *
 * 架构总纲 §4a — 离线层规划
 * 技术选型：vite-plugin-pwa + idb
 *
 * Service Worker 缓存策略：
 *   - Precache:          app shell (JS/CSS/HTML)
 *   - Network First:     API 调用（30s 超时，回退缓存）
 *   - Stale While Revalidate: 静态资源（图片、字体）
 *   - Cache First:       字体文件（不可变资源，年度缓存）
 *
 * 离线存储（IndexedDB）：
 *   - market-snapshots:  行情快照离线缓存
 *   - operation-queue:   操作队列（断线暂存，恢复重放）
 *   - cache-metadata:    缓存元数据
 *
 * @module infrastructure/offline
 */

export {
  getOfflineDb,
  closeOfflineDb,
  deleteOfflineDb,
} from './db';
export type {
  MarketSnapshotRecord,
  OperationQueueRecord,
  CacheMetadataRecord,
  OfflineDb,
} from './db';

export {
  saveMarketSnapshot,
  saveMarketSnapshots,
  getMarketSnapshot,
  getMarketSnapshotsBySymbol,
  pruneOldSnapshots,
  getCachedSymbols,
} from './market-snapshot';

export {
  enqueueOperation,
  getPendingOperations,
  markProcessing,
  markCompleted,
  markFailed,
  resetFailedOperation,
  getQueueStats,
  pruneOldOperations,
} from './operation-queue';

/**
 * 检查浏览器是否支持 PWA 所需的 API
 */
export function isPwaSupported(): boolean {
  return (
    'serviceWorker' in navigator &&
    'IndexedDB' in window &&
    (window.indexedDB as IDBFactory | undefined) !== undefined
  );
}

/**
 * 检查当前是否处于离线状态
 */
export function isOffline(): boolean {
  return !navigator.onLine;
}

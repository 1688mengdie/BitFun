/**
 * 离线操作队列
 *
 * 断线时操作暂存队列，上线后自动重放。
 * 类似 CQRS 的 command queue 模式。
 *
 * @module infrastructure/offline/operation-queue
 */

import { getOfflineDb, type OperationQueueRecord } from './db';

/**
 * 入队一个离线操作
 */
export async function enqueueOperation(
  type: string,
  payload: unknown,
): Promise<string> {
  const db = await getOfflineDb();

  // 简易 UUID v4 生成（生产环境应使用 crypto.randomUUID）
  const id = crypto.randomUUID
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;

  const record: OperationQueueRecord = {
    id,
    type,
    payload,
    createdAt: Date.now(),
    status: 'pending',
    retryCount: 0,
  };

  await db.add('operation-queue', record);
  return id;
}

/**
 * 获取所有待处理的操作（按创建时间排序）
 */
export async function getPendingOperations(): Promise<OperationQueueRecord[]> {
  const db = await getOfflineDb();
  const all = await db.getAllFromIndex('operation-queue', 'status_created', [
    'pending',
  ]);
  return all.sort((a, b) => a.createdAt - b.createdAt);
}

/**
 * 标记操作开始处理
 */
export async function markProcessing(id: string): Promise<void> {
  const db = await getOfflineDb();
  const record = await db.get('operation-queue', id);
  if (!record) return;

  record.status = 'processing';
  await db.put('operation-queue', record);
}

/**
 * 标记操作完成
 */
export async function markCompleted(id: string): Promise<void> {
  const db = await getOfflineDb();
  const record = await db.get('operation-queue', id);
  if (!record) return;

  record.status = 'completed';
  await db.put('operation-queue', record);
}

/**
 * 标记操作失败
 */
export async function markFailed(
  id: string,
  error: string,
): Promise<void> {
  const db = await getOfflineDb();
  const record = await db.get('operation-queue', id);
  if (!record) return;

  record.status = 'failed';
  record.retryCount += 1;
  record.lastError = error;
  await db.put('operation-queue', record);
}

/**
 * 重置失败操作为待处理（用于重试）
 */
export async function resetFailedOperation(id: string): Promise<void> {
  const db = await getOfflineDb();
  const record = await db.get('operation-queue', id);
  if (!record) return;

  record.status = 'pending';
  await db.put('operation-queue', record);
}

/**
 * 获取队列统计
 */
export async function getQueueStats(): Promise<{
  pending: number;
  processing: number;
  completed: number;
  failed: number;
}> {
  const db = await getOfflineDb();
  const all = await db.getAll('operation-queue');
  return {
    pending: all.filter((r) => r.status === 'pending').length,
    processing: all.filter((r) => r.status === 'processing').length,
    completed: all.filter((r) => r.status === 'completed').length,
    failed: all.filter((r) => r.status === 'failed').length,
  };
}

/**
 * 清理已完成/失败的操作（保留最近 N 条）
 */
export async function pruneOldOperations(
  keepCount: number = 100,
): Promise<number> {
  const db = await getOfflineDb();
  const all = await db.getAllFromIndex('operation-queue', 'createdAt');

  // 按时间降序排列，保留最新的 keepCount 条
  const sorted = all.sort((a, b) => b.createdAt - a.createdAt);
  const toDelete = sorted.slice(keepCount);

  const tx = db.transaction('operation-queue', 'readwrite');
  for (const op of toDelete) {
    await tx.store.delete(op.id);
  }
  await tx.done;

  return toDelete.length;
}

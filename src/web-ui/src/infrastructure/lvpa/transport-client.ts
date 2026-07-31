/**
 * LVPA TransportClient — 消费后端数据的统一 SDK。
 *
 * 基于 ITransportAdapter 封装，提供领域特定接口。
 * 构造函数接受 ITransportAdapter，通过依赖注入支持测试 mock。
 *
 * 对应 Phase-6-类型契约.md §四。
 */

import type { ITransportAdapter } from '@/infrastructure/api/adapters/base';
import type {
  AgentStatus,
  KlineTick,
  Observable,
  Task,
  TaskEvent,
  TaskFilter,
  WorkshopProgress,
} from './types';

// ── 内部 SimpleObservable 实现 ──

class SimpleObservable<T> implements Observable<T> {
  constructor(
    private readonly subscribeFn: (handler: (value: T) => void) => () => void,
  ) {}

  subscribe(handler: (value: T) => void): () => void {
    return this.subscribeFn(handler);
  }
}

// ── 活动订阅追踪 ──

interface ActiveKlineSubscription {
  symbol: string;
  period: string;
  unsubscribe: () => void;
}

// ── LvpaTransportClient ──

export class LvpaTransportClient {
  private activeKlineSubscriptions: Map<string, ActiveKlineSubscription> = new Map();

  constructor(private readonly adapter: ITransportAdapter) {}

  // ── 连接管理 ──

  async connect(): Promise<void> {
    return this.adapter.connect();
  }

  async disconnect(): Promise<void> {
    // 清理所有活动订阅
    for (const [, sub] of this.activeKlineSubscriptions) {
      sub.unsubscribe();
    }
    this.activeKlineSubscriptions.clear();

    return this.adapter.disconnect();
  }

  isConnected(): boolean {
    return this.adapter.isConnected();
  }

  // ── K线数据 ──

  /**
   * 订阅 K线 Tick 流。
   * 返回 AsyncGenerator，消费者通过 for await...of 消费。
   * 消费者 break/return 时自动取消订阅。
   */
  async *subscribeKline(symbol: string, period: string): AsyncGenerator<KlineTick> {
    const eventName = `kline:${symbol}:${period}`;
    const buffer: KlineTick[] = [];
    let pendingResolve: ((value: KlineTick) => void) | null = null;

    const unsubscribe = this.adapter.listen<KlineTick>(eventName, (tick) => {
      if (pendingResolve) {
        const resolve = pendingResolve;
        pendingResolve = null;
        resolve(tick);
      } else {
        buffer.push(tick);
      }
    });

    // 注册到活动订阅表
    const subKey = `${symbol}:${period}`;
    this.activeKlineSubscriptions.set(subKey, { symbol, period, unsubscribe });

    try {
      while (true) {
        if (buffer.length > 0) {
          yield buffer.shift()!;
        } else {
          yield await new Promise<KlineTick>((resolve) => {
            pendingResolve = resolve;
          });
        }
      }
    } finally {
      this.activeKlineSubscriptions.delete(subKey);
      unsubscribe();
    }
  }

  /** 取消 K线 订阅（对应 subscribeKline 的主动取消） */
  unsubscribeKline(symbol: string, period?: string): void {
    // 如果提供了 period 则只取消特定订阅，否则取消该 symbol 的所有订阅
    const prefix = period ? `${symbol}:${period}` : `${symbol}:`;

    for (const [key, sub] of this.activeKlineSubscriptions) {
      if (key === prefix || key.startsWith(prefix)) {
        sub.unsubscribe();
        this.activeKlineSubscriptions.delete(key);
      }
    }
  }

  // ── Agent 状态 ──

  async getAgentStatus(agentId: string): Promise<AgentStatus> {
    return this.adapter.request<AgentStatus>('get_agent_status', { agentId });
  }

  watchAgentStatus(agentId: string): Observable<AgentStatus> {
    const eventName = `agent:${agentId}:status`;
    return new SimpleObservable<AgentStatus>((handler) => {
      return this.adapter.listen<AgentStatus>(eventName, (data) => handler(data));
    });
  }

  // ── 工坊进度 ──

  watchWorkshopProgress(workshopId: string): Observable<WorkshopProgress> {
    const eventName = `workshop:${workshopId}:progress`;
    return new SimpleObservable<WorkshopProgress>((handler) => {
      return this.adapter.listen<WorkshopProgress>(eventName, (data) => handler(data));
    });
  }

  // ── 任务大厅 ──

  async getTaskList(filter: TaskFilter): Promise<Task[]> {
    return this.adapter.request<Task[]>('get_task_list', { filter });
  }

  watchTaskUpdates(): Observable<TaskEvent> {
    return new SimpleObservable<TaskEvent>((handler) => {
      return this.adapter.listen<TaskEvent>('task:updates', (data) => handler(data));
    });
  }

  // ── 通用事件 ──

  /**
   * 通用事件订阅。
   * 返回 unsubscribe 函数。
   */
  onEvent(eventName: string, handler: (payload: any) => void): () => void {
    return this.adapter.listen(eventName, handler);
  }
}

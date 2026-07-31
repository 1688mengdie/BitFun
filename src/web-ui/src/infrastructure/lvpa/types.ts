/**
 * LVPA TransportClient 领域事件类型。
 *
 * 对应 Phase-6-类型契约.md §四。
 * 供 LvpaTransportClient 封装消费。
 */

/** K线数据 Tick */
export interface KlineTick {
  symbol: string;
  period: string;
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  /** 持仓量（期货） */
  oi?: number;
}

/** Agent 状态 */
export interface AgentStatus {
  agentId: string;
  name: string;
  /** 境界 */
  realm: string;
  /** 灵根 */
  spiritRoot: string;
  /** 评分 */
  credit: number;
  /** 灵石 */
  spiritStones: number;
  /** 当前任务 */
  currentTask?: string;
  state: 'idle' | 'working' | 'blocked' | 'error';
}

/** 工坊进度 */
export interface WorkshopProgress {
  workshopId: string;
  name: string;
  currentTask: string;
  /** 0-100 */
  progressPct: number;
  status: 'running' | 'paused' | 'completed' | 'error';
  startedAt: number;
  estimatedEndAt?: number;
}

/** 任务 */
export interface Task {
  taskId: string;
  title: string;
  type: 'trade' | 'analysis' | 'research' | 'monitor' | 'system';
  priority: 1 | 2 | 3 | 4 | 5;
  status: 'open' | 'assigned' | 'in_progress' | 'review' | 'done';
  /** 灵石 */
  reward: number;
  assignedAgent?: string;
  createdAt: number;
}

/** 任务事件 */
export interface TaskEvent {
  type: 'created' | 'assigned' | 'progress' | 'completed' | 'cancelled';
  task: Task;
  timestamp: number;
}

/** 任务过滤条件 */
export interface TaskFilter {
  status?: Task['status'][];
  type?: Task['type'][];
  priority?: Task['priority'][];
  assignedTo?: string;
}

/**
 * 最小 Observable 接口 — 用于 watch* 方法。
 *
 * subscribe 返回 unsubscribe 函数。
 */
export interface Observable<T> {
  subscribe(handler: (value: T) => void): () => void;
}

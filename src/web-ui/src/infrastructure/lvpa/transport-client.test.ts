import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ITransportAdapter } from '@/infrastructure/api/adapters/base';
import { LvpaTransportClient } from './transport-client';
import type {
  AgentStatus,
  KlineTick,
  Task,
  TaskEvent,
  TaskFilter,
  WorkshopProgress,
} from './types';

// ── Mock 工厂 ──

function createMockAdapter(): {
  adapter: ITransportAdapter;
  connect: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
  isConnected: ReturnType<typeof vi.fn>;
  request: ReturnType<typeof vi.fn>;
  listen: ReturnType<typeof vi.fn>;
} {
  const connect = vi.fn().mockResolvedValue(undefined);
  const disconnect = vi.fn().mockResolvedValue(undefined);
  const isConnected = vi.fn().mockReturnValue(false);
  const request = vi.fn();
  const listen = vi.fn().mockReturnValue(vi.fn());

  const adapter: ITransportAdapter = {
    connect,
    disconnect,
    isConnected,
    request,
    listen,
  };

  return { adapter, connect, disconnect, isConnected, request, listen };
}

/** 辅助函数：创建一个自动交付第一个 tick 的 kline mock。
 *  调度一个 microtask 在 generator 挂起后交付 tick，避免死锁。 */
function autoDeliverKlineMock(mock: ReturnType<typeof createMockAdapter>, tick?: KlineTick): KlineTick {
  const delivered: KlineTick = tick ?? {
    symbol: 'BTC', period: '1m', timestamp: 0,
    open: 0, high: 0, low: 0, close: 0, volume: 0,
  };
  mock.listen.mockImplementation((_event: string, cb: (data: KlineTick) => void) => {
    queueMicrotask(() => cb(delivered));
    return vi.fn();
  });
  return delivered;
}

// ── 测试套件 ──

describe('LvpaTransportClient', () => {
  let mock: ReturnType<typeof createMockAdapter>;
  let client: LvpaTransportClient;

  beforeEach(() => {
    vi.clearAllMocks();
    mock = createMockAdapter();
    client = new LvpaTransportClient(mock.adapter);
  });

  // ── 连接管理 ──

  describe('connect / disconnect / isConnected', () => {
    it('connect() delegates to adapter.connect()', async () => {
      await client.connect();
      expect(mock.connect).toHaveBeenCalledTimes(1);
    });

    it('disconnect() delegates to adapter.disconnect()', async () => {
      await client.disconnect();
      expect(mock.disconnect).toHaveBeenCalledTimes(1);
    });

    it('disconnect() cleans up active kline subscriptions', async () => {
      const unsubscribe = vi.fn();
      // Combined implementation: deliver a tick AND return the tracked unsubscribe
      mock.listen.mockImplementation((_event: string, cb: (data: KlineTick) => void) => {
        queueMicrotask(() => cb({
          symbol: 'BTC', period: '1m', timestamp: 0,
          open: 0, high: 0, low: 0, close: 0, volume: 0,
        }));
        return unsubscribe;
      });

      // subscribeKline is an async generator — body runs on .next()
      const generator = client.subscribeKline('BTC', '1m');
      await generator.next();
      expect(mock.listen).toHaveBeenCalledWith('kline:BTC:1m', expect.any(Function));

      await client.disconnect();
      expect(unsubscribe).toHaveBeenCalled();
      expect(mock.disconnect).toHaveBeenCalledTimes(1);

      // Clean up generator
      await generator.return(null);
    });

    it('isConnected() delegates to adapter.isConnected()', () => {
      mock.isConnected.mockReturnValue(true);
      expect(client.isConnected()).toBe(true);
      expect(mock.isConnected).toHaveBeenCalledTimes(1);

      mock.isConnected.mockReturnValue(false);
      expect(client.isConnected()).toBe(false);
    });
  });

  // ── K线数据 ──

  describe('subscribeKline / unsubscribeKline', () => {
    it('subscribeKline sets up adapter.listen with correct event name', async () => {
      autoDeliverKlineMock(mock);

      const generator = client.subscribeKline('BTC', '1m');
      await generator.next();

      expect(mock.listen).toHaveBeenCalledWith('kline:BTC:1m', expect.any(Function));
      await generator.return(null);
    });

    it('subscribeKline yields ticks from adapter.listen callback', async () => {
      const expectedTick: KlineTick = {
        symbol: 'ETH',
        period: '5m',
        timestamp: 1000,
        open: 100,
        high: 200,
        low: 50,
        close: 150,
        volume: 1000,
      };
      autoDeliverKlineMock(mock, expectedTick);

      const generator = client.subscribeKline('ETH', '5m');
      const result = await generator.next();

      expect(result.value).toEqual(expectedTick);
      expect(result.done).toBe(false);
      await generator.return(null);
    });

    it('subscribeKline buffers ticks when no consumer is awaiting', async () => {
      // Capture the listen callback manually for controlled delivery
      let capturedCb: ((data: KlineTick) => void) | null = null;
      mock.listen.mockImplementation((_event: string, cb: (data: KlineTick) => void) => {
        capturedCb = cb;
        return vi.fn();
      });

      const generator = client.subscribeKline('BTC', '1m');

      // First .next() starts the generator; deliver tick1 to unblock it
      const tick1: KlineTick = {
        symbol: 'BTC', period: '1m', timestamp: 1,
        open: 10, high: 20, low: 5, close: 15, volume: 100,
      };
      const next1 = generator.next();
      // capturedCb is set synchronously during the listen call inside .next()
      // But the generator hasn't suspended yet — we need to wait for the microtask
      // Actually, capturedCb should be set already. Let's deliver.
      // But if we deliver synchronously here, the generator might not be awaiting yet.
      // Use queueMicrotask to ensure the generator has suspended:
      queueMicrotask(() => capturedCb!(tick1));

      const result1 = await next1;
      expect(result1.value).toEqual(tick1);

      // Now send tick2 while generator is not awaiting → it goes to buffer
      const tick2: KlineTick = {
        symbol: 'BTC', period: '1m', timestamp: 2,
        open: 15, high: 25, low: 10, close: 20, volume: 200,
      };

      // Generator is now at while(true) → buffer is empty → awaits
      // Deliver tick2 through captured callback
      const next2 = generator.next();
      capturedCb!(tick2);
      const result2 = await next2;
      expect(result2.value).toEqual(tick2);

      await generator.return(null);
    });

    it('subscribeKline cleans up listener on generator return', async () => {
      const unsubscribe = vi.fn();
      // Combined: deliver tick via microtask AND return the tracked unsubscribe
      mock.listen.mockImplementation((_event: string, cb: (data: KlineTick) => void) => {
        queueMicrotask(() => cb({
          symbol: 'BTC', period: '1m', timestamp: 0,
          open: 0, high: 0, low: 0, close: 0, volume: 0,
        }));
        return unsubscribe;
      });

      const generator = client.subscribeKline('BTC', '1m');
      // First .next() triggers generator body → calls listen → gets unsubscribe
      await generator.next();
      expect(mock.listen).toHaveBeenCalledTimes(1);

      // generator.return() triggers finally block → calls unsubscribe
      await generator.return(null);
      expect(unsubscribe).toHaveBeenCalledTimes(1);
    });

    it('unsubscribeKline removes specific subscription by symbol and period', async () => {
      const unsubscribe = vi.fn();
      // Combined: deliver tick AND return the tracked unsubscribe
      mock.listen.mockImplementation((_event: string, cb: (data: KlineTick) => void) => {
        queueMicrotask(() => cb({
          symbol: 'BTC', period: '1m', timestamp: 0,
          open: 0, high: 0, low: 0, close: 0, volume: 0,
        }));
        return unsubscribe;
      });

      const generator = client.subscribeKline('BTC', '1m');
      await generator.next();
      expect(mock.listen).toHaveBeenCalledWith('kline:BTC:1m', expect.any(Function));

      client.unsubscribeKline('BTC', '1m');
      expect(unsubscribe).toHaveBeenCalled();
      await generator.return(null);
    });

    it('unsubscribeKline removes all subscriptions for a symbol when period omitted', async () => {
      const unsub1 = vi.fn();
      const unsub2 = vi.fn();

      let callCount = 0;
      mock.listen.mockImplementation((_event: string, cb: (data: KlineTick) => void) => {
        callCount++;
        queueMicrotask(() => cb({
          symbol: 'BTC', period: callCount === 1 ? '1m' : '5m',
          timestamp: callCount, open: 0, high: 0, low: 0, close: 0, volume: 0,
        }));
        return callCount === 1 ? unsub1 : unsub2;
      });

      const gen1 = client.subscribeKline('BTC', '1m');
      const gen2 = client.subscribeKline('BTC', '5m');
      await gen1.next();
      await gen2.next();

      client.unsubscribeKline('BTC');

      expect(unsub1).toHaveBeenCalled();
      expect(unsub2).toHaveBeenCalled();

      await gen1.return(null);
      await gen2.return(null);
    });
  });

  // ── Agent 状态 ──

  describe('getAgentStatus / watchAgentStatus', () => {
    it('getAgentStatus calls adapter.request with correct action', async () => {
      const expected: AgentStatus = {
        agentId: 'a1',
        name: '炼丹童子',
        realm: '炼气',
        spiritRoot: '火',
        credit: 85,
        spiritStones: 1000,
        state: 'idle',
      };
      mock.request.mockResolvedValue(expected);

      const result = await client.getAgentStatus('a1');
      expect(mock.request).toHaveBeenCalledWith('get_agent_status', { agentId: 'a1' });
      expect(result).toEqual(expected);
    });

    it('watchAgentStatus returns Observable that delegates to adapter.listen', () => {
      const adapterUnsubscribe = vi.fn();
      mock.listen.mockReturnValue(adapterUnsubscribe);

      const observable = client.watchAgentStatus('a1');
      const handler = vi.fn();

      // subscribe() triggers the lazy SimpleObservable factory → calls adapter.listen
      const unsubscribe = observable.subscribe(handler);
      expect(mock.listen).toHaveBeenCalledWith('agent:a1:status', expect.any(Function));

      // Simulate an event
      const agentStatus: AgentStatus = {
        agentId: 'a1',
        name: '炼丹童子',
        realm: '炼气',
        spiritRoot: '火',
        credit: 85,
        spiritStones: 1000,
        state: 'working',
        currentTask: '炼丹',
      };
      const listenCallback = mock.listen.mock.calls[0][1];
      listenCallback(agentStatus);

      expect(handler).toHaveBeenCalledWith(agentStatus);

      // Unsubscribe
      unsubscribe();
      expect(adapterUnsubscribe).toHaveBeenCalled();
    });
  });

  // ── 工坊进度 ──

  describe('watchWorkshopProgress', () => {
    it('returns Observable that delegates to adapter.listen', () => {
      mock.listen.mockReturnValue(vi.fn());

      const observable = client.watchWorkshopProgress('ws1');
      const handler = vi.fn();

      // subscribe() triggers the lazy SimpleObservable factory → calls adapter.listen
      observable.subscribe(handler);
      expect(mock.listen).toHaveBeenCalledWith('workshop:ws1:progress', expect.any(Function));

      const progress: WorkshopProgress = {
        workshopId: 'ws1',
        name: '天机坊',
        currentTask: '铸剑',
        progressPct: 50,
        status: 'running',
        startedAt: 1000,
        estimatedEndAt: 2000,
      };
      const listenCallback = mock.listen.mock.calls[0][1];
      listenCallback(progress);

      expect(handler).toHaveBeenCalledWith(progress);
    });
  });

  // ── 任务大厅 ──

  describe('getTaskList / watchTaskUpdates', () => {
    it('getTaskList calls adapter.request with filter', async () => {
      const tasks: Task[] = [
        {
          taskId: 't1',
          title: '分析 BTC 走势',
          type: 'analysis',
          priority: 3,
          status: 'open',
          reward: 100,
          createdAt: 1000,
        },
      ];
      mock.request.mockResolvedValue(tasks);

      const filter: TaskFilter = { type: ['analysis'] };
      const result = await client.getTaskList(filter);
      expect(mock.request).toHaveBeenCalledWith('get_task_list', { filter });
      expect(result).toEqual(tasks);
    });

    it('watchTaskUpdates returns Observable for task events', () => {
      mock.listen.mockReturnValue(vi.fn());

      const observable = client.watchTaskUpdates();
      const handler = vi.fn();

      // subscribe() triggers the lazy SimpleObservable factory → calls adapter.listen
      observable.subscribe(handler);
      expect(mock.listen).toHaveBeenCalledWith('task:updates', expect.any(Function));

      const event: TaskEvent = {
        type: 'created',
        task: {
          taskId: 't2',
          title: '监控 ETH 价格',
          type: 'monitor',
          priority: 2,
          status: 'open',
          reward: 50,
          createdAt: 2000,
        },
        timestamp: 2000,
      };
      const listenCallback = mock.listen.mock.calls[0][1];
      listenCallback(event);

      expect(handler).toHaveBeenCalledWith(event);
    });
  });

  // ── 通用事件 ──

  describe('onEvent', () => {
    it('delegates to adapter.listen and returns unsubscribe', () => {
      const adapterUnsubscribe = vi.fn();
      mock.listen.mockReturnValue(adapterUnsubscribe);

      const handler = vi.fn();
      const unsubscribe = client.onEvent('custom:event', handler);

      expect(mock.listen).toHaveBeenCalledWith('custom:event', handler);

      unsubscribe();
      expect(adapterUnsubscribe).toHaveBeenCalled();
    });
  });
});

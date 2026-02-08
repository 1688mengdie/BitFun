/**
 * Event batcher
 * 
 * Uses requestAnimationFrame to batch high-frequency events and reduce UI updates
 * 
 * Design principles:
 * - Events with the same key are merged (accumulated or replaced)
 * - Batch processing triggered once per frame
 * - Supports different key generation strategies for normal and subagent events
 */

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('EventBatcher');

export type MergeStrategy = 'accumulate' | 'replace';

export interface BatchedEvent<T = any> {
  key: string;
  payload: T;
  strategy: MergeStrategy;
  accumulator?: (existing: T, incoming: T) => T;
  timestamp: number;
}

export interface EventBatcherOptions {
  onFlush: (events: Array<{ key: string; payload: any }>) => void;
  debug?: boolean;
}

export class EventBatcher {
  private buffer: Map<string, BatchedEvent> = new Map();
  private scheduled = false;
  private onFlush: (events: Array<{ key: string; payload: any }>) => void;
  private debug: boolean;
  private frameId: number | null = null;

  // Update frequency control to prevent UI blocking with many events
  private UPDATE_INTERVAL = 100; // Update every 100ms instead of every frame (16.67ms)
  private lastUpdateTime = 0;
  private timeoutId: ReturnType<typeof setTimeout> | null = null;

  constructor(options: EventBatcherOptions) {
    this.onFlush = options.onFlush;
    this.debug = options.debug ?? false;
  }

  add<T>(
    key: string,
    payload: T,
    strategy: MergeStrategy = 'replace',
    accumulator?: (existing: T, incoming: T) => T
  ): void {
    const existing = this.buffer.get(key);

    if (existing) {
      if (strategy === 'accumulate' && accumulator) {
        existing.payload = accumulator(existing.payload, payload);
        existing.timestamp = Date.now();
      } else {
        existing.payload = payload;
        existing.timestamp = Date.now();
      }

      if (this.debug) {
        log.debug('Merged event', { key, strategy });
      }
    } else {
      this.buffer.set(key, {
        key,
        payload,
        strategy,
        accumulator,
        timestamp: Date.now()
      });

      if (this.debug) {
        log.debug('Added new event', { key, strategy });
      }
    }

    this.scheduleFlush();
  }

  private scheduleFlush(): void {
    if (this.scheduled) return;
    this.scheduled = true;

    const now = performance.now();
    const timeSinceLastUpdate = now - this.lastUpdateTime;

    if (timeSinceLastUpdate >= this.UPDATE_INTERVAL) {
      this.frameId = requestAnimationFrame(() => {
        this.flush();
        this.scheduled = false;
        this.frameId = null;
        this.lastUpdateTime = performance.now();
      });
    } else {
      const delay = this.UPDATE_INTERVAL - timeSinceLastUpdate;
      this.timeoutId = setTimeout(() => {
        this.frameId = requestAnimationFrame(() => {
          this.flush();
          this.scheduled = false;
          this.frameId = null;
          this.lastUpdateTime = performance.now();
        });
        this.timeoutId = null;
      }, delay);
    }
  }

  private flush(): void {
    if (this.buffer.size === 0) return;

    const startTime = performance.now();
    const bufferSize = this.buffer.size;

    const events = Array.from(this.buffer.values()).map(({ key, payload }) => ({
      key,
      payload
    }));

    if (this.debug) {
      log.debug('Flushing batched events', { count: events.length });
    }

    this.buffer = new Map();
    this.onFlush(events);

    const duration = performance.now() - startTime;
    if (this.debug || duration > 10) {
      log.warn('Event batch processing took longer than expected', { 
        eventCount: bufferSize, 
        duration: duration.toFixed(2) 
      });
    }
    if (duration > 16.67) {
      log.error('Event batch processing exceeded frame time', { 
        eventCount: bufferSize, 
        duration: duration.toFixed(2),
        frameTime: 16.67
      });
    }
  }

  flushNow(): void {
    if (this.frameId !== null) {
      cancelAnimationFrame(this.frameId);
      this.frameId = null;
    }
    if (this.timeoutId !== null) {
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }
    this.scheduled = false;
    this.flush();
  }

  getBufferSize(): number {
    return this.buffer.size;
  }

  clear(): void {
    if (this.frameId !== null) {
      cancelAnimationFrame(this.frameId);
      this.frameId = null;
    }
    if (this.timeoutId !== null) {
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }
    this.buffer.clear();
    this.scheduled = false;

    if (this.debug) {
      log.debug('Buffer cleared');
    }
  }

  destroy(): void {
    this.clear();
  }
}

export interface SubagentParentInfo {
  sessionId: string;
  toolCallId: string;
  dialogTurnId: string;
}

export interface TextChunkEventData {
  sessionId: string;
  turnId: string;
  roundId: string;
  text: string;
  contentType: 'text' | 'thinking';
  subagentParentInfo?: SubagentParentInfo;
}

export interface ToolEventData {
  sessionId: string;
  turnId: string;
  toolEvent: {
    tool_id: string;
    eventType: string;
    [key: string]: any;
  };
  subagentParentInfo?: SubagentParentInfo;
}

/**
 * Generate merge key for TextChunk events
 * 
 * Key structure:
 * - Normal text: text:{sessionId}:{roundId}:{contentType}
 * - Subagent text: subagent:text:{parentSessionId}:{parentToolId}:{subSessionId}:{roundId}:{contentType}
 */
export function generateTextChunkKey(data: TextChunkEventData): string {
  const { sessionId, roundId, contentType, subagentParentInfo } = data;

  if (subagentParentInfo) {
    const { sessionId: parentSessionId, toolCallId: parentToolId } = subagentParentInfo;
    return `subagent:text:${parentSessionId}:${parentToolId}:${sessionId}:${roundId}:${contentType}`;
  } else {
    return `text:${sessionId}:${roundId}:${contentType}`;
  }
}

/**
 * Generate merge key for ToolEvent events
 * 
 * Returns null if the event doesn't need batching (isolated event)
 * 
 * Key structure:
 * - Tool params: tool:params:{sessionId}:{toolUseId}
 * - Subagent tool params: subagent:tool:params:{parentSessionId}:{parentToolId}:{subToolUseId}
 * - Tool progress: tool:progress:{sessionId}:{toolUseId}
 * - Subagent tool progress: subagent:tool:progress:{parentSessionId}:{parentToolId}:{subToolUseId}
 */
export function generateToolEventKey(data: ToolEventData): { key: string; strategy: MergeStrategy } | null {
  const { sessionId, toolEvent, subagentParentInfo } = data;
  const { tool_id: toolUseId, eventType } = toolEvent;

  const isolatedEvents = ['Detected', 'Started', 'Completed', 'Failed', 'Cancelled', 'ConfirmationNeeded'];
  if (isolatedEvents.includes(eventType)) {
    return null;
  }

  if (subagentParentInfo) {
    const { sessionId: parentSessionId, toolCallId: parentToolId } = subagentParentInfo;

    if (eventType === 'ParamsPartial') {
      return {
        key: `subagent:tool:params:${parentSessionId}:${parentToolId}:${toolUseId}`,
        strategy: 'accumulate'
      };
    }
    if (eventType === 'Progress') {
      return {
        key: `subagent:tool:progress:${parentSessionId}:${parentToolId}:${toolUseId}`,
        strategy: 'replace'
      };
    }
  } else {
    if (eventType === 'ParamsPartial') {
      return {
        key: `tool:params:${sessionId}:${toolUseId}`,
        strategy: 'accumulate'
      };
    }
    if (eventType === 'Progress') {
      return {
        key: `tool:progress:${sessionId}:${toolUseId}`,
        strategy: 'replace'
      };
    }
  }

  return null;
}

/**
 * Parse event key to extract event type information
 */
export function parseEventKey(key: string): {
  isSubagent: boolean;
  eventType: 'text' | 'tool:params' | 'tool:progress';
  ids: Record<string, string>;
} | null {
  const parts = key.split(':');

  if (parts[0] === 'subagent') {
    // subagent:text:parentSessionId:parentToolId:subSessionId:roundId:contentType
    // subagent:tool:params:parentSessionId:parentToolId:subToolUseId
    // subagent:tool:progress:parentSessionId:parentToolId:subToolUseId
    if (parts[1] === 'text') {
      return {
        isSubagent: true,
        eventType: 'text',
        ids: {
          parentSessionId: parts[2],
          parentToolId: parts[3],
          subSessionId: parts[4],
          roundId: parts[5],
          contentType: parts[6]
        }
      };
    } else if (parts[1] === 'tool' && parts[2] === 'params') {
      return {
        isSubagent: true,
        eventType: 'tool:params',
        ids: {
          parentSessionId: parts[3],
          parentToolId: parts[4],
          subToolUseId: parts[5]
        }
      };
    } else if (parts[1] === 'tool' && parts[2] === 'progress') {
      return {
        isSubagent: true,
        eventType: 'tool:progress',
        ids: {
          parentSessionId: parts[3],
          parentToolId: parts[4],
          subToolUseId: parts[5]
        }
      };
    }
  } else {
    // text:sessionId:roundId:contentType
    // tool:params:sessionId:toolUseId
    // tool:progress:sessionId:toolUseId
    if (parts[0] === 'text') {
      return {
        isSubagent: false,
        eventType: 'text',
        ids: {
          sessionId: parts[1],
          roundId: parts[2],
          contentType: parts[3]
        }
      };
    } else if (parts[0] === 'tool' && parts[1] === 'params') {
      return {
        isSubagent: false,
        eventType: 'tool:params',
        ids: {
          sessionId: parts[2],
          toolUseId: parts[3]
        }
      };
    } else if (parts[0] === 'tool' && parts[1] === 'progress') {
      return {
        isSubagent: false,
        eventType: 'tool:progress',
        ids: {
          sessionId: parts[2],
          toolUseId: parts[3]
        }
      };
    }
  }

  return null;
}


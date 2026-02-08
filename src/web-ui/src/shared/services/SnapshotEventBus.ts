/**
 * Snapshot event bus.
 *
 * Bridges backend snapshot events into a lightweight pub/sub API for the UI.
 * Supports global listeners and per-session listeners.
 */
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SnapshotEventBus');

type UnlistenFn = () => void;

type EventCallback = (data: any) => void;

/**
 * Event shape emitted by the backend snapshot subsystem.
 */
export interface SnapshotEvent {
  type: string;
  data: any;
}

export class SnapshotEventBus {
  private sessionListeners: Map<string, Map<string, Set<EventCallback>>>;
  private globalListeners: Map<string, Set<EventCallback>>;
  private tauriUnlisteners: UnlistenFn[];
  private initialized: boolean;

  constructor() {
    this.sessionListeners = new Map();
    this.globalListeners = new Map();
    this.tauriUnlisteners = [];
    this.initialized = false;
  }

  /**
   * Initializes backend event listeners once. Safe to call multiple times.
   */
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }

    try {
      const unlisten = api.listen<SnapshotEvent>('snapshot-event', (event) => {
        const { type, data } = event;
        if (data.session_id) {
          this.emit(data.session_id, type, data);
        }
        this.emitGlobal(type, data);
      });
      this.tauriUnlisteners.push(unlisten);
      this.initialized = true;
      log.info('Initialized and listening for backend events');
    } catch (error) {
      log.error('Failed to initialize', error);
    }
  }

  /**
   * Subscribes to events scoped to a specific session.
   * Returns an unsubscribe function.
   */
  subscribe(sessionId: string, eventType: string, callback: EventCallback): () => void {
    if (!this.initialized) {
      this.initialize().catch((err) => log.error('Failed to initialize', err));
    }

    if (!this.sessionListeners.has(sessionId)) {
      this.sessionListeners.set(sessionId, new Map());
      this.setupSessionTauriListener(sessionId).catch((err) => log.error('Failed to setup session listener', { sessionId, error: err }));
    }
    
    const sessionEvents = this.sessionListeners.get(sessionId)!;
    if (!sessionEvents.has(eventType)) {
      sessionEvents.set(eventType, new Set());
    }
    
    sessionEvents.get(eventType)!.add(callback);

    return () => {
      const listeners = sessionEvents.get(eventType);
      if (listeners) {
        listeners.delete(callback);
        if (listeners.size === 0) {
          sessionEvents.delete(eventType);
        }
      }
      if (sessionEvents.size === 0) {
        this.sessionListeners.delete(sessionId);
      }
    };
  }

   
  private async setupSessionTauriListener(sessionId: string): Promise<void> {
    try {
      const eventName = `snapshot-event:${sessionId}`;
      const unlisten = await listen<SnapshotEvent>(eventName, (event) => {
        const { type, data } = event.payload;
        this.emit(sessionId, type, data);
      });
      
      this.tauriUnlisteners.push(unlisten);
    } catch (error) {
      log.error('Failed to setup session listener', { sessionId, error });
    }
  }

  /**
   * Subscribes to global (non-session-scoped) snapshot events.
   * Returns an unsubscribe function.
   */
  subscribeGlobal(eventType: string, callback: EventCallback): () => void {
    if (!this.initialized) {
      this.initialize().catch((err) => log.error('Failed to initialize', err));
    }

    if (!this.globalListeners.has(eventType)) {
      this.globalListeners.set(eventType, new Set());
    }
    
    this.globalListeners.get(eventType)!.add(callback);

    return () => {
      const listeners = this.globalListeners.get(eventType);
      if (listeners) {
        listeners.delete(callback);
        if (listeners.size === 0) {
          this.globalListeners.delete(eventType);
        }
      }
    };
  }

  /**
   * Emits a session-scoped event to local listeners.
   */
  emit(sessionId: string, eventType: string, data: any): void {
    
    const sessionEvents = this.sessionListeners.get(sessionId);
    if (sessionEvents) {
      const listeners = sessionEvents.get(eventType);
      if (listeners && listeners.size > 0) {
        listeners.forEach(callback => {
          try {
            callback(data);
          } catch (error) {
            log.error('Error in listener', { sessionId, eventType, error });
          }
        });
      }
    }
  }

   
  private emitGlobal(eventType: string, data: any): void {
    const globalListeners = this.globalListeners.get(eventType);
    if (globalListeners) {
      globalListeners.forEach(callback => {
        try {
          callback(data);
        } catch (error) {
          log.error('Error in global listener', { eventType, error });
        }
      });
    }
  }

   
  clearSession(sessionId: string): void {
    this.sessionListeners.delete(sessionId);
  }

   
  clearAll(): void {
    this.sessionListeners.clear();
    this.globalListeners.clear();
    
    
    this.tauriUnlisteners.forEach(unlisten => unlisten());
    this.tauriUnlisteners = [];
    this.initialized = false;
  }

   
  destroy(): void {
    this.clearAll();
  }
}


export const snapshotEventBus = new SnapshotEventBus();


snapshotEventBus.initialize().catch((err) => log.error('Failed to auto-initialize', err));

/**
 * Terminal hook for state and event handling.
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { getTerminalService, TerminalService } from '../services';
import { createLogger } from '@/shared/utils/logger';
import type {
  SessionResponse,
  TerminalEvent,
  TerminalEventCallback,
} from '../types';

const log = createLogger('useTerminal');

export interface UseTerminalOptions {
  sessionId: string;
  autoConnect?: boolean;
  onEvent?: TerminalEventCallback;
  onOutput?: (data: string) => void;
  onReady?: () => void;
  onExit?: (exitCode?: number) => void;
  onError?: (message: string) => void;
}

export interface UseTerminalReturn {
  service: TerminalService;
  session: SessionResponse | null;
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;
  write: (data: string) => Promise<void>;
  resize: (cols: number, rows: number) => Promise<void>;
  sendCtrlC: () => Promise<void>;
  close: () => Promise<void>;
  refresh: () => Promise<void>;
}

export function useTerminal(options: UseTerminalOptions): UseTerminalReturn {
  const {
    sessionId,
    autoConnect = true,
    onEvent,
    onOutput,
    onReady,
    onExit,
    onError,
  } = options;

  const [session, setSession] = useState<SessionResponse | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const serviceRef = useRef<TerminalService>(getTerminalService());
  const unsubscribeRef = useRef<(() => void) | null>(null);

  const handleEvent = useCallback((event: TerminalEvent) => {
    if (event.sessionId !== sessionId) return;

    onEvent?.(event);

    switch (event.type) {
      case 'output':
        onOutput?.((event as any).data);
        break;
      case 'ready':
        onReady?.();
        break;
      case 'exit':
        onExit?.((event as any).exitCode);
        break;
      case 'error':
        onError?.((event as any).message);
        setError((event as any).message);
        break;
      case 'resize':
        // Backend resize confirmation requires no extra UI work.
        break;
    }
  }, [sessionId, onEvent, onOutput, onReady, onExit, onError]);

  useEffect(() => {
    const service = serviceRef.current;

    const connect = async () => {
      try {
        setIsLoading(true);
        setError(null);

        if (autoConnect && !service.isConnected()) {
          await service.connect();
        }
        setIsConnected(service.isConnected());

        // Subscribe before fetching session data to avoid missing events.
        unsubscribeRef.current = service.onSessionEvent(sessionId, handleEvent);

        try {
          const history = await service.getHistory(sessionId);
          if (history && history.data) {
            onOutput?.(history.data);
          }
        } catch (historyErr) {
          log.warn('Failed to get history, continuing connection', { sessionId, error: historyErr });
        }

        const sessionInfo = await service.getSession(sessionId);
        setSession(sessionInfo);

        setIsLoading(false);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Connection failed';
        setError(message);
        setIsLoading(false);
        log.error('Failed to connect', { sessionId, error: err });
      }
    };

    connect();

    return () => {
      if (unsubscribeRef.current) {
        unsubscribeRef.current();
        unsubscribeRef.current = null;
      }
    };
  }, [sessionId, autoConnect, handleEvent]);

  const write = useCallback(async (data: string) => {
    try {
      await serviceRef.current.write(sessionId, data);
    } catch (err) {
      log.error('Failed to write', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  const resize = useCallback(async (cols: number, rows: number) => {
    try {
      await serviceRef.current.resize(sessionId, cols, rows);
    } catch (err) {
      log.error('Failed to resize', { sessionId, cols, rows, error: err });
      throw err;
    }
  }, [sessionId]);

  const sendCtrlC = useCallback(async () => {
    try {
      await serviceRef.current.sendCtrlC(sessionId);
    } catch (err) {
      log.error('Failed to send Ctrl+C', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  const close = useCallback(async () => {
    try {
      await serviceRef.current.closeSession(sessionId);
      setSession(null);
    } catch (err) {
      log.error('Failed to close session', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  const refresh = useCallback(async () => {
    try {
      const sessionInfo = await serviceRef.current.getSession(sessionId);
      setSession(sessionInfo);
    } catch (err) {
      log.error('Failed to refresh session', { sessionId, error: err });
      throw err;
    }
  }, [sessionId]);

  return {
    service: serviceRef.current,
    session,
    isConnected,
    isLoading,
    error,
    write,
    resize,
    sendCtrlC,
    close,
    refresh,
  };
}


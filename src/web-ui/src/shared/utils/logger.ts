/**
 * Unified logging utility
 * Provides leveled logging with @tauri-apps/plugin-log backend
 * Falls back to console in non-Tauri environments
 */

import {
  trace as tauriTrace,
  debug as tauriDebug,
  info as tauriInfo,
  warn as tauriWarn,
  error as tauriError,
  attachConsole,
} from '@tauri-apps/plugin-log';

export enum LogLevel {
  TRACE = 0,
  DEBUG = 1,
  INFO = 2,
  WARN = 3,
  ERROR = 4,
  NONE = 5,
}

export interface LogEntry {
  level: LogLevel;
  message: string;
  timestamp: Date;
  context?: string;
  data?: any;
}

// Check if running in Tauri environment
const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
const isDev = import.meta.env?.DEV ?? process.env.NODE_ENV === 'development';

// Logger initialization state
let initialized = false;
let initPromise: Promise<void> | null = null;

/**
 * Initialize logger - attaches console listener in dev mode
 * Call this once at app startup
 */
export async function initLogger(): Promise<void> {
  if (initialized) return;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    if (isTauri && isDev) {
      try {
        await attachConsole();
      } catch (e) {
        console.warn('[Logger] Failed to attach console:', e);
      }
    }
    initialized = true;
  })();

  return initPromise;
}

/**
 * Format data for logging
 * Separates Error objects from data: JSON for regular data, stack trace appended separately
 */
function formatData(data: unknown): string {
  if (data === undefined || data === null) return '';
  if (data instanceof Error) {
    return data.stack || data.message;
  }
  if (typeof data === 'object') {
    try {
      // Separate Error objects from regular data
      const regularData: Record<string, unknown> = {};
      const errors: string[] = [];

      for (const key in data as Record<string, unknown>) {
        const value = (data as Record<string, unknown>)[key];
        if (value instanceof Error) {
          errors.push(value.stack || `${value.name}: ${value.message}`);
        } else {
          regularData[key] = value;
        }
      }

      const parts: string[] = [];
      if (Object.keys(regularData).length > 0) {
        parts.push(JSON.stringify(regularData));
      }
      if (errors.length > 0) {
        parts.push(errors.join('\n'));
      }

      return parts.join(', ');
    } catch {
      return String(data);
    }
  }
  return String(data);
}

export class Logger {
  private static instance: Logger;
  private currentLevel: LogLevel;

  private constructor() {
    this.currentLevel = isDev ? LogLevel.DEBUG : LogLevel.WARN;
  }

  public static getInstance(): Logger {
    if (!Logger.instance) {
      Logger.instance = new Logger();
    }
    return Logger.instance;
  }

  public setLevel(level: LogLevel): void {
    this.currentLevel = level;
  }

  public getLevel(): LogLevel {
    return this.currentLevel;
  }

  public trace(message: string, context?: string, data?: any): void {
    this.log(LogLevel.TRACE, message, context, data);
  }

  public debug(message: string, context?: string, data?: any): void {
    this.log(LogLevel.DEBUG, message, context, data);
  }

  public info(message: string, context?: string, data?: any): void {
    this.log(LogLevel.INFO, message, context, data);
  }

  public warn(message: string, context?: string, data?: any): void {
    this.log(LogLevel.WARN, message, context, data);
  }

  public error(message: string, context?: string, data?: any): void {
    this.log(LogLevel.ERROR, message, context, data);
  }

  private log(level: LogLevel, message: string, context?: string, data?: any): void {
    if (level < this.currentLevel) {
      return;
    }

    const logEntry: LogEntry = {
      level,
      message,
      timestamp: new Date(),
      context,
      data,
    };

    this.output(logEntry);
  }

  private output(entry: LogEntry): void {
    const { level, message, context, data } = entry;
    const contextStr = context ? `[${context}] ` : '';
    const dataStr = formatData(data);
    const fullMessage = dataStr ? `${contextStr}${message} ${dataStr}` : `${contextStr}${message}`;

    if (isTauri) {
      this.outputTauri(level, fullMessage);
    } else {
      this.outputConsole(level, fullMessage, data);
    }
  }

  private outputTauri(level: LogLevel, message: string): void {
    // Fire and forget - don't await to avoid blocking
    switch (level) {
      case LogLevel.TRACE:
        tauriTrace(message).catch(() => {});
        break;
      case LogLevel.DEBUG:
        tauriDebug(message).catch(() => {});
        break;
      case LogLevel.INFO:
        tauriInfo(message).catch(() => {});
        break;
      case LogLevel.WARN:
        tauriWarn(message).catch(() => {});
        break;
      case LogLevel.ERROR:
        tauriError(message).catch(() => {});
        break;
    }
  }

  private outputConsole(level: LogLevel, message: string, data?: any): void {
    const args = data !== undefined ? [message, data] : [message];
    switch (level) {
      case LogLevel.TRACE:
      case LogLevel.DEBUG:
        console.debug(...args);
        break;
      case LogLevel.INFO:
        console.info(...args);
        break;
      case LogLevel.WARN:
        console.warn(...args);
        break;
      case LogLevel.ERROR:
        console.error(...args);
        break;
    }
  }

  public createContextLogger(context: string) {
    return {
      trace: (message: string, data?: any) => this.trace(message, context, data),
      debug: (message: string, data?: any) => this.debug(message, context, data),
      info: (message: string, data?: any) => this.info(message, context, data),
      warn: (message: string, data?: any) => this.warn(message, context, data),
      error: (message: string, data?: any) => this.error(message, context, data),
    };
  }
}

export const logger = Logger.getInstance();

export const createLogger = (context: string) => logger.createContextLogger(context);

export const log = {
  trace: (message: string, context?: string, data?: any) => logger.trace(message, context, data),
  debug: (message: string, context?: string, data?: any) => logger.debug(message, context, data),
  info: (message: string, context?: string, data?: any) => logger.info(message, context, data),
  warn: (message: string, context?: string, data?: any) => logger.warn(message, context, data),
  error: (message: string, context?: string, data?: any) => logger.error(message, context, data),
};

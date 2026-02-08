/**
 * Centralized terminal action manager.
 * Keeps a fixed number of EventBus listeners regardless of instance count.
 * Adds multi-line paste confirmation similar to VS Code.
 */

import { Terminal as XTerm } from '@xterm/xterm';
import { globalEventBus } from '@/infrastructure/event-bus';
import { confirmWarning } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TerminalActionManager');

/** Line threshold for multi-line paste confirmation. */
const MULTILINE_PASTE_THRESHOLD = 1;

export interface TerminalActionHandler {
  getTerminal: () => XTerm | null;
  /** Read-only terminals cannot paste or clear. */
  isReadOnly?: boolean;
  write?: (data: string) => Promise<void> | void;
  clear?: () => void;
}

class TerminalActionManager {
  private static instance: TerminalActionManager;
  
  private handlers = new Map<string, TerminalActionHandler>();
  
  private unsubscribers: (() => void)[] = [];
  
  private initialized = false;

  private constructor() {
  }

  static getInstance(): TerminalActionManager {
    if (!TerminalActionManager.instance) {
      TerminalActionManager.instance = new TerminalActionManager();
    }
    return TerminalActionManager.instance;
  }

  init(): void {
    if (this.initialized) {
      return;
    }

    const unsubCopy = globalEventBus.on('terminal:copy', this.handleCopy);
    
    const unsubPaste = globalEventBus.on('terminal:paste', this.handlePaste);
    
    const unsubSelectAll = globalEventBus.on('terminal:select-all', this.handleSelectAll);
    
    const unsubClear = globalEventBus.on('terminal:clear', this.handleClear);

    this.unsubscribers = [unsubCopy, unsubPaste, unsubSelectAll, unsubClear];
    this.initialized = true;
  }

  destroy(): void {
    this.unsubscribers.forEach(unsub => unsub());
    this.unsubscribers = [];
    this.handlers.clear();
    this.initialized = false;
  }

  register(terminalId: string, handler: TerminalActionHandler): void {
    if (!this.initialized) {
      this.init();
    }

    this.handlers.set(terminalId, handler);
    log.debug('Terminal registered', { terminalId, total: this.handlers.size });
  }

  unregister(terminalId: string): void {
    const deleted = this.handlers.delete(terminalId);
    if (deleted) {
      log.debug('Terminal unregistered', { terminalId, total: this.handlers.size });
    }
  }

  getRegisteredCount(): number {
    return this.handlers.size;
  }

  private handleCopy = async (data: { terminalId: string }): Promise<void> => {
    const handler = this.handlers.get(data.terminalId);
    if (!handler) {
      return;
    }

    const terminal = handler.getTerminal();
    if (!terminal) {
      return;
    }

    const selection = terminal.getSelection();
    if (selection) {
      try {
        await navigator.clipboard.writeText(selection);
      } catch (err) {
        log.error('Copy failed', { terminalId: data.terminalId, error: err });
      }
    }
  };

  /**
   * Paste handler with multi-line confirmation.
   */
  private handlePaste = async (data: { terminalId: string }): Promise<void> => {
    const handler = this.handlers.get(data.terminalId);
    if (!handler) {
      return;
    }

    if (handler.isReadOnly || !handler.write) {
      return;
    }

    try {
      const text = await navigator.clipboard.readText();
      if (!text) {
        return;
      }

      const lines = text.split('\n');
      const lineCount = lines.length;
      
      if (lineCount > MULTILINE_PASTE_THRESHOLD) {
        const maxPreviewLines = 10;
        const previewLines = lines.slice(0, maxPreviewLines);
        let preview = previewLines.join('\n');
        if (lineCount > maxPreviewLines) {
          preview += `\n... (${lineCount} lines total)`;
        }
        
        const confirmed = await confirmWarning(
          'Paste multiple lines',
          `The clipboard contains ${lineCount} lines. Pasting multiple lines in a terminal may execute multiple commands.`,
          {
            confirmText: 'Paste',
            cancelText: 'Cancel',
            preview,
            previewMaxHeight: 150,
          }
        );

        if (!confirmed) {
          return;
        }
      }

      await handler.write(text);
      
    } catch (err) {
      log.error('Paste failed', { terminalId: data.terminalId, error: err });
    }
  };

  private handleSelectAll = (data: { terminalId: string }): void => {
    const handler = this.handlers.get(data.terminalId);
    if (!handler) {
      return;
    }

    const terminal = handler.getTerminal();
    if (terminal) {
      terminal.selectAll();
    }
  };

  private handleClear = (data: { terminalId: string }): void => {
    const handler = this.handlers.get(data.terminalId);
    if (!handler) {
      return;
    }

    if (handler.isReadOnly || !handler.clear) {
      return;
    }

    handler.clear();
  };
}

export const terminalActionManager = TerminalActionManager.getInstance();

export function registerTerminalActions(terminalId: string, handler: TerminalActionHandler): void {
  terminalActionManager.register(terminalId, handler);
}

export function unregisterTerminalActions(terminalId: string): void {
  terminalActionManager.unregister(terminalId);
}

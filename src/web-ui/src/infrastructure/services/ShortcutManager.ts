 

import { ShortcutConfig } from '@/shared/types/prompt-template';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ShortcutManager');

export type ShortcutCallback = (event: KeyboardEvent) => void;

interface ShortcutRegistration {
  id: string;
  config: ShortcutConfig;
  callback: ShortcutCallback;
  description?: string;
  priority: number; 
}

 
export class ShortcutManager {
  private static instance: ShortcutManager;
  private registrations: Map<string, ShortcutRegistration> = new Map();
  private keyDownHandler: ((e: KeyboardEvent) => void) | null = null;
  private isEnabled: boolean = true;

  private constructor() {
    this.start();
  }

   
  public static getInstance(): ShortcutManager {
    if (!ShortcutManager.instance) {
      ShortcutManager.instance = new ShortcutManager();
    }
    return ShortcutManager.instance;
  }

   
  private start(): void {
    if (this.keyDownHandler) {
      return;
    }

    this.keyDownHandler = this.handleKeyDown.bind(this);
    window.addEventListener('keydown', this.keyDownHandler, true);
  }

   
  public stop(): void {
    if (this.keyDownHandler) {
      window.removeEventListener('keydown', this.keyDownHandler, true);
      this.keyDownHandler = null;
    }
  }

   
  public register(
    id: string,
    config: ShortcutConfig,
    callback: ShortcutCallback,
    options?: {
      description?: string;
      priority?: number;
    }
  ): () => void {
    
    const conflicts = this.checkConflicts(config);
    if (conflicts.length > 0) {
      log.warn('Shortcut conflict detected', { 
        shortcut: this.formatShortcut(config), 
        conflicts: conflicts.map(c => c.id) 
      });
    }

    const registration: ShortcutRegistration = {
      id,
      config,
      callback,
      description: options?.description,
      priority: options?.priority ?? 0
    };

    this.registrations.set(id, registration);

    
    return () => this.unregister(id);
  }

   
  public unregister(id: string): boolean {
    return this.registrations.delete(id);
  }

   
  private handleKeyDown(event: KeyboardEvent): void {
    if (!this.isEnabled) {
      return;
    }

    
    if (this.shouldIgnoreEvent(event)) {
      return;
    }

    
    const matched = this.findMatchingShortcuts(event);

    if (matched.length === 0) {
      return;
    }

    
    matched.sort((a, b) => b.priority - a.priority);

    
    const registration = matched[0];
    
    try {
      event.preventDefault();
      event.stopPropagation();
      registration.callback(event);
    } catch (error) {
      log.error('Shortcut callback execution failed', { id: registration.id, error });
    }
  }

   
  private findMatchingShortcuts(event: KeyboardEvent): ShortcutRegistration[] {
    const matched: ShortcutRegistration[] = [];

    for (const registration of this.registrations.values()) {
      if (this.isShortcutMatch(event, registration.config)) {
        matched.push(registration);
      }
    }

    return matched;
  }

   
  private isShortcutMatch(event: KeyboardEvent, config: ShortcutConfig): boolean {
    
    const key = event.key.toUpperCase();
    const configKey = config.key.toUpperCase();
    
    if (key !== configKey) {
      return false;
    }

    
    if (!!config.ctrl !== (event.ctrlKey || event.metaKey)) {
      return false;
    }

    if (!!config.shift !== event.shiftKey) {
      return false;
    }

    if (!!config.alt !== event.altKey) {
      return false;
    }

    if (!!config.meta !== event.metaKey) {
      return false;
    }

    return true;
  }

   
  private shouldIgnoreEvent(event: KeyboardEvent): boolean {
    const target = event.target as HTMLElement;
    if (!target) {
      return false;
    }

    
    const isCodeEditor = target.classList.contains('monaco-editor') ||
                        target.classList.contains('code-editor') ||
                        target.closest('.monaco-editor') !== null ||
                        target.closest('.code-editor') !== null;
    
    if (isCodeEditor) {
      return false;
    }

    const tagName = target.tagName.toLowerCase();

    
    if (['input', 'textarea', 'select'].includes(tagName)) {
      const style = window.getComputedStyle(target);
      if (style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0') {
        return true;
      }
    }

    
    if (target.classList.contains('bitfun-chat-input') ||
        target.classList.contains('rich-text-input') ||
        target.closest('.bitfun-chat-input') !== null ||
        target.closest('.rich-text-input') !== null) {
      return true;
    }

    
    if (target.isContentEditable) {
      return true;
    }

    return false;
  }

   
  public checkConflicts(config: ShortcutConfig): ShortcutRegistration[] {
    const conflicts: ShortcutRegistration[] = [];

    for (const registration of this.registrations.values()) {
      if (this.isShortcutEqual(config, registration.config)) {
        conflicts.push(registration);
      }
    }

    return conflicts;
  }

   
  private isShortcutEqual(a: ShortcutConfig, b: ShortcutConfig): boolean {
    return (
      a.key.toUpperCase() === b.key.toUpperCase() &&
      !!a.ctrl === !!b.ctrl &&
      !!a.shift === !!b.shift &&
      !!a.alt === !!b.alt &&
      !!a.meta === !!b.meta
    );
  }

   
  public formatShortcut(config: ShortcutConfig): string {
    const parts: string[] = [];

    if (config.ctrl) parts.push('Ctrl');
    if (config.shift) parts.push('Shift');
    if (config.alt) parts.push('Alt');
    if (config.meta) parts.push('Meta');
    parts.push(config.key.toUpperCase());

    return parts.join('+');
  }

   
  public parseShortcut(shortcut: string): ShortcutConfig | null {
    const parts = shortcut.split('+').map(s => s.trim().toLowerCase());
    
    if (parts.length === 0) {
      return null;
    }

    const key = parts[parts.length - 1];
    
    return {
      key: key.toUpperCase(),
      ctrl: parts.includes('ctrl'),
      shift: parts.includes('shift'),
      alt: parts.includes('alt'),
      meta: parts.includes('meta')
    };
  }

   
  public validateShortcut(shortcut: string): boolean {
    return this.parseShortcut(shortcut) !== null;
  }

   
  public getAllRegistrations(): ShortcutRegistration[] {
    return Array.from(this.registrations.values());
  }

   
  public setEnabled(enabled: boolean): void {
    this.isEnabled = enabled;
  }

   
  public isShortcutEnabled(): boolean {
    return this.isEnabled;
  }

   
  public clear(): void {
    this.registrations.clear();
  }
}


export const shortcutManager = ShortcutManager.getInstance();


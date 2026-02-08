 

import {
  IConfigManager,
  ConfigValidationResult,
  ConfigExport,
} from '../types';
import { configAPI } from '@/infrastructure/api';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ConfigManager');

class ConfigManagerImpl implements IConfigManager {
  
  private configCache: Map<string, any> = new Map();
  private listeners: Set<(path: string, oldValue: any, newValue: any) => void> = new Set();
  private pathListeners: Map<string, Set<() => void>> = new Map();

  constructor() {
    log.info('Initializing config manager (proxy mode)');
  }

  

  async getConfig<T = any>(path?: string): Promise<T> {
    try {
      
      if (path && this.configCache.has(path)) {
        return this.configCache.get(path);
      }

      
      const config = await configAPI.getConfig(path);

      
      if (path) {
        this.configCache.set(path, config);
      }
      
      return config;
    } catch (error) {
      log.error('Failed to get config', { path, error });
      // Return defaults to avoid breaking the UI.
      if (path === 'ai.models') {
        return [] as T;
      }
      if (path === 'ai.agent_models') {
        return {} as T;
      }
      if (path === 'ai.func_agent_models') {
        return {} as T;
      }
      if (path === 'ai.default_models') {
        return {} as T;
      }
      throw error;
    }
  }

  async setConfig<T = any>(path: string, value: T): Promise<void> {
    try {
      const oldValue = this.configCache.get(path);
      
      
      await configAPI.setConfig(path, value);
      
      
      this.configCache.set(path, value);
      
      
      this.notifyConfigChange(path, oldValue, value);
    } catch (error) {
      log.error('Failed to set config', { path, error });
      throw error;
    }
  }

  async resetConfig(path?: string): Promise<void> {
    try {
      await configAPI.resetConfig(path);
      
      
      if (path) {
        this.configCache.delete(path);
      } else {
        this.configCache.clear();
      }
    } catch (error) {
      log.error('Failed to reset config', { path, error });
      throw error;
    }
  }

  async validateConfig(): Promise<ConfigValidationResult> {
    try {
      
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<ConfigValidationResult>('validate_config');
      return result;
    } catch (error) {
      log.error('Failed to validate config', error);
      return {
        valid: false,
        errors: [{ path: 'root', message: i18nService.t('errors:config.validationError'), code: 'VALIDATION_ERROR' }],
        warnings: []
      };
    }
  }

  async exportConfig(): Promise<ConfigExport> {
    try {
      const exportData = await configAPI.exportConfig();
      return exportData;
    } catch (error) {
      log.error('Failed to export config', error);
      throw error;
    }
  }

  async importConfig(config: ConfigExport): Promise<void> {
    try {
      await configAPI.importConfig(config);
      
      
      this.configCache.clear();
    } catch (error) {
      log.error('Failed to import config', error);
      throw error;
    }
  }

  

  onConfigChange(callback: (path: string, oldValue: any, newValue: any) => void): () => void {
    this.listeners.add(callback);
    return () => {
      this.listeners.delete(callback);
    };
  }

  async refreshCache(): Promise<void> {
    try {
      this.configCache.clear();
    } catch (error) {
      log.error('Failed to refresh cache', error);
    }
  }

  clearCache(): void {
    this.configCache.clear();
  }

  
  private notifyConfigChange(path: string, oldValue: any, newValue: any): void {
    this.listeners.forEach(callback => {
      try {
        callback(path, oldValue, newValue);
      } catch (error) {
        log.error('Config change notification failed', { path, error });
      }
    });
    
    
    const pathCallbacks = this.pathListeners.get(path);
    if (pathCallbacks) {
      pathCallbacks.forEach(callback => {
        try {
          callback();
        } catch (error) {
          log.error('Path listener notification failed', { path, error });
        }
      });
    }
  }

  
  
  
  get<T = any>(path: string, defaultValue?: T): T {
    if (this.configCache.has(path)) {
      const value = this.configCache.get(path);
      return value !== undefined ? value : (defaultValue as T);
    }
    return defaultValue as T;
  }
  
  
  async set<T = any>(path: string, value: T): Promise<void> {
    return this.setConfig(path, value);
  }
  
  
  watch(path: string, callback: () => void): () => void {
    if (!this.pathListeners.has(path)) {
      this.pathListeners.set(path, new Set());
    }
    
    const pathCallbacks = this.pathListeners.get(path)!;
    pathCallbacks.add(callback);
    
    
    return () => {
      pathCallbacks.delete(callback);
      if (pathCallbacks.size === 0) {
        this.pathListeners.delete(path);
      }
    };
  }
  
  
  async reload(): Promise<void> {
    try {
      this.configCache.clear();
      
      await this.getConfig('ai.models');
      await this.getConfig('ai.agent_models');
      await this.getConfig('ai.func_agent_models');
      await this.getConfig('ai.default_models');
    } catch (error) {
      log.error('Failed to reload config', error);
      throw error;
    }
  }
}


export const configManager = new ConfigManagerImpl();

export default configManager;
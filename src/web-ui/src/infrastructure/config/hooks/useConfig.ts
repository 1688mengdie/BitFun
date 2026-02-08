 

import { useState, useEffect, useCallback } from 'react';
import { UseConfigReturn, GlobalConfig, ConfigValidationResult } from '../types';
import { configManager } from '../services/ConfigManager';

export function useConfig<T = GlobalConfig>(path?: string): UseConfigReturn<T> {
  const [config, setConfig] = useState<T>(() => 
    path ? configManager.get<T>(path, {} as T) : configManager.getAll() as T
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasChanges, setHasChanges] = useState(false);

  
  useEffect(() => {
    const unsubscribe = configManager.watch(path || '', (newValue: T, oldValue: T) => {
      setConfig(newValue);
      setHasChanges(configManager.hasChanges());
    });

    
    setHasChanges(configManager.hasChanges());

    return unsubscribe;
  }, [path]);

  
  const updateConfig = useCallback(async (updates: Partial<T>): Promise<void> => {
    try {
      setLoading(true);
      setError(null);

      if (path) {
        
        const currentConfig = configManager.get<T>(path, {} as T);
        const newConfig = { ...currentConfig, ...updates };
        await configManager.set(path, newConfig);
      } else {
        
        const updatePaths: Record<string, any> = {};
        for (const [key, value] of Object.entries(updates)) {
          updatePaths[key] = value;
        }
        await configManager.update(updatePaths);
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to update config';
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [path]);

  
  const resetConfig = useCallback(async (): Promise<void> => {
    try {
      setLoading(true);
      setError(null);
      await configManager.reset(path);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to reset config';
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [path]);

  
  const saveConfig = useCallback(async (): Promise<void> => {
    try {
      setLoading(true);
      setError(null);
      await configManager.save();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to save config';
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, []);

  
  const reloadConfig = useCallback(async (): Promise<void> => {
    try {
      setLoading(true);
      setError(null);
      await configManager.reload();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to reload config';
      setError(errorMessage);
      throw err;
    } finally {
      setLoading(false);
    }
  }, []);

  
  const validateConfig = useCallback((): ConfigValidationResult => {
    if (path) {
      const pathConfig = configManager.get(path);
      return configManager.validate({ [path]: pathConfig } as any);
    }
    return configManager.validate();
  }, [path]);

  return {
    config,
    loading,
    error,
    hasChanges,
    updateConfig,
    resetConfig,
    saveConfig,
    reloadConfig,
    validateConfig
  };
}


export function useAppConfig() {
  return useConfig<GlobalConfig['app']>('app');
}



export function useEditorConfig() {
  return useConfig<GlobalConfig['editor']>('editor');
}

export function useTerminalConfig() {
  return useConfig<GlobalConfig['terminal']>('terminal');
}

export function useWorkspaceConfig() {
  return useConfig<GlobalConfig['workspace']>('workspace');
}

export function useAIConfig() {
  return useConfig<GlobalConfig['ai']>('ai');
}


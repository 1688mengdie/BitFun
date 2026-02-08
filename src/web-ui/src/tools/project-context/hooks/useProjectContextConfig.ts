/**
 * Project Context configuration hook.
 * Manages module enablement and priority.
 */

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useProjectContextConfig');

export type ProjectContextModule = 'docs' | 'rules' | 'architecture' | 'knowledge';

export interface ModuleState {
  enabled: boolean;
  priority: number;
  enabledCount?: number;
  totalCount?: number;
  items?: Record<string, { enabled: boolean; tokenEstimate?: number }>;
}

export interface ProjectContextConfig {
  tokenBudget: number;
  modules: Record<ProjectContextModule, ModuleState>;
}

const DEFAULT_CONFIG: ProjectContextConfig = {
  tokenBudget: 20000,
  modules: {
    docs: { enabled: true, priority: 3, enabledCount: 0, totalCount: 0 },
    rules: { enabled: true, priority: 4, enabledCount: 0, totalCount: 0 },
    architecture: { enabled: true, priority: 2 },
    knowledge: { enabled: true, priority: 3, enabledCount: 0, totalCount: 0 }
  }
};

export function useProjectContextConfig(workspacePath: string) {
  const [config, setConfig] = useState<ProjectContextConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadConfig = useCallback(async () => {
    if (!workspacePath) {
      setLoading(false);
      return;
    }

    setLoading(true);
    try {
      const savedConfig = await invoke<ProjectContextConfig | null>('get_project_context_config', {
        workspacePath
      }).catch(() => null);

      if (savedConfig) {
        setConfig(savedConfig);
      } else {
        setConfig(DEFAULT_CONFIG);
      }
      setError(null);
    } catch (err) {
      log.error('Failed to load config', { workspacePath, error: err });
      setError(err instanceof Error ? err.message : 'Failed to load config');
      setConfig(DEFAULT_CONFIG);
    } finally {
      setLoading(false);
    }
  }, [workspacePath]);

  const saveConfig = useCallback(async (newConfig: ProjectContextConfig) => {
    if (!workspacePath) return;

    try {
      await invoke('save_project_context_config', {
        workspacePath,
        config: newConfig
      }).catch(() => {
        // Backend API may be missing; keep local state only.
        log.warn('Backend save failed, saving locally only', { workspacePath });
      });
    } catch (err) {
      log.error('Failed to save config', { workspacePath, error: err });
    }
  }, [workspacePath]);

  const toggleModule = useCallback((module: ProjectContextModule) => {
    setConfig(prev => {
      const newConfig = {
        ...prev,
        modules: {
          ...prev.modules,
          [module]: {
            ...prev.modules[module],
            enabled: !prev.modules[module].enabled
          }
        }
      };
      saveConfig(newConfig);
      return newConfig;
    });
  }, [saveConfig]);

  const updatePriority = useCallback((module: ProjectContextModule, priority: number) => {
    setConfig(prev => {
      const newConfig = {
        ...prev,
        modules: {
          ...prev.modules,
          [module]: {
            ...prev.modules[module],
            priority
          }
        }
      };
      saveConfig(newConfig);
      return newConfig;
    });
  }, [saveConfig]);

  const updateModuleItem = useCallback((
    module: ProjectContextModule, 
    itemId: string, 
    enabled: boolean
  ) => {
    setConfig(prev => {
      const moduleState = prev.modules[module];
      const items = moduleState.items || {};
      const newItems = {
        ...items,
        [itemId]: { ...items[itemId], enabled }
      };
      
      // Recompute counts after a single item change.
      const enabledCount = Object.values(newItems).filter(i => i.enabled).length;
      
      const newConfig = {
        ...prev,
        modules: {
          ...prev.modules,
          [module]: {
            ...moduleState,
            items: newItems,
            enabledCount,
            totalCount: Object.keys(newItems).length
          }
        }
      };
      saveConfig(newConfig);
      return newConfig;
    });
  }, [saveConfig]);

  const updateTokenBudget = useCallback((budget: number) => {
    setConfig(prev => {
      const newConfig = { ...prev, tokenBudget: budget };
      saveConfig(newConfig);
      return newConfig;
    });
  }, [saveConfig]);

  const getTokenUsage = useCallback(() => {
    let total = 0;
    
    for (const moduleKey of Object.keys(config.modules) as ProjectContextModule[]) {
      const moduleState = config.modules[moduleKey];
      if (!moduleState.enabled) continue;
      
      if (moduleState.items) {
        for (const item of Object.values(moduleState.items)) {
          if (item.enabled && item.tokenEstimate) {
            total += item.tokenEstimate;
          }
        }
      }
    }
    
    return total;
  }, [config]);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  return {
    config,
    loading,
    error,
    toggleModule,
    updatePriority,
    updateModuleItem,
    updateTokenBudget,
    getTokenUsage,
    reload: loadConfig
  };
}

export default useProjectContextConfig;

import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw, ListChecks, X, Save } from 'lucide-react';
import { IconButton, Card, Switch, Select } from '@/component-library';
import { notificationService } from '@/shared/notification-system';
import { configAPI } from '@/infrastructure/api';
import type { ModeConfigItem } from '../types';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { createLogger } from '@/shared/utils/logger';
import './AgenticModeConfig.scss';

const log = createLogger('AgenticModeConfig');

interface AgenticPreferences {
  visualMode: boolean;
  priorityStrategy: 'economy' | 'quality' | 'balanced';
}

interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
}

interface AgenticModeConfigProps {
  embedded?: boolean;
}

export const AgenticModeConfig: React.FC<AgenticModeConfigProps> = ({ embedded = false }) => {
  const { t } = useTranslation('settings/super-agent');
  const [loading, setLoading] = useState(false);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [agenticConfig, setAgenticConfig] = useState<ModeConfigItem | null>(null);
  
  const [preferences, setPreferences] = useState<AgenticPreferences>({
    visualMode: false,
    priorityStrategy: 'balanced',
  });
  
  const [toolsCollapsed, setToolsCollapsed] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);
      
      const [toolsData, modeConfig] = await Promise.all([
        fetchAvailableTools(),
        configAPI.getModeConfig('agentic')
      ]);
      
      setAvailableTools(toolsData);
      setAgenticConfig(modeConfig);
      
      log.debug('Data loaded', { toolsCount: toolsData.length, agenticConfig: modeConfig });
    } catch (error) {
      log.error('Failed to load data', { error });
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      notificationService.error(`${t('messages.loadFailed')}: ${errorMsg}`, {
        duration: 3000
      });
    } finally {
      setLoading(false);
    }
  };

  const fetchAvailableTools = async (): Promise<ToolInfo[]> => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const tools = await invoke<ToolInfo[]>('get_all_tools_info');
      return tools;
    } catch (error) {
      log.error('Failed to fetch tools list', { error });
      return [];
    }
  };

  const toggleTool = async (toolName: string) => {
    if (!agenticConfig) return;
    
    const tools = agenticConfig.available_tools || [];
    const isEnabling = !tools.includes(toolName);
    
    const newTools = isEnabling
      ? [...tools, toolName]
      : tools.filter(t => t !== toolName);
    
    setAgenticConfig({
      ...agenticConfig,
      available_tools: newTools
    });
  };

  const selectAllTools = () => {
    if (!agenticConfig) return;
    setAgenticConfig({
      ...agenticConfig,
      available_tools: availableTools.map(t => t.name)
    });
  };

  const clearAllTools = () => {
    if (!agenticConfig) return;
    setAgenticConfig({
      ...agenticConfig,
      available_tools: []
    });
  };

  const saveToolsConfig = async () => {
    if (!agenticConfig) return;
    
    try {
      setLoading(true);
      await configAPI.setModeConfig('agentic', agenticConfig);
      notificationService.success(t('messages.saveSuccess'));
      
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      log.debug('Mode config update event emitted');
    } catch (error) {
      log.error('Failed to save tools config', { error });
      notificationService.error(`${t('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error)));
    } finally {
      setLoading(false);
    }
  };

  const resetToolsConfig = async () => {
    if (!await window.confirm(t('messages.confirmReset'))) {
      return;
    }

    try {
      setLoading(true);
      await configAPI.resetModeConfig('agentic');
      await loadData();
      notificationService.success(t('messages.resetSuccess'));
    } catch (error) {
      log.error('Failed to reset tools config', { error });
      notificationService.error(t('messages.resetFailed'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <ConfigPageLayout className="agentic-mode-config">
      <ConfigPageHeader
        title={t('tools.title', 'Agentic Tools Configuration')}
        subtitle={t('tools.subtitle', 'Configure available tools for Agentic mode')}
      />

      <ConfigPageContent className="agentic-mode-config__content">
        <div className="agentic-mode-config__preferences-section">
          <div className="agentic-mode-config__preferences-content">
            <div className="agentic-mode-config__preference-item">
              <div className="agentic-mode-config__preference-info">
                <span className="agentic-mode-config__preference-label">
                  {t('preferences.visualMode.label', 'Visual Mode')}
                </span>
                <span className="agentic-mode-config__preference-desc">
                  {t('preferences.visualMode.description', 'Enable visual feedback during execution')}
                </span>
              </div>
              <Switch
                checked={preferences.visualMode}
                onChange={(checked) => setPreferences(prev => ({ ...prev, visualMode: checked }))}
                size="small"
              />
            </div>
            
            <div className="agentic-mode-config__preference-item">
              <div className="agentic-mode-config__preference-info">
                <span className="agentic-mode-config__preference-label">
                  {t('preferences.priorityStrategy.label', 'Priority Strategy')}
                </span>
                <span className="agentic-mode-config__preference-desc">
                  {t('preferences.priorityStrategy.description', 'Control model selection and invocation strategy')}
                </span>
              </div>
              <div className="agentic-mode-config__preference-select">
                <Select
                  value={preferences.priorityStrategy}
                  onChange={(value) => setPreferences(prev => ({ ...prev, priorityStrategy: value as AgenticPreferences['priorityStrategy'] }))}
                  options={[
                    { value: 'economy', label: t('preferences.priorityStrategy.options.economy', 'Economy First') },
                    { value: 'balanced', label: t('preferences.priorityStrategy.options.balanced', 'Balanced') },
                    { value: 'quality', label: t('preferences.priorityStrategy.options.quality', 'Quality First') },
                  ]}
                  size="small"
                />
              </div>
            </div>
          </div>
        </div>

        {agenticConfig && (
          <div className={`agentic-mode-config__tools-section ${!toolsCollapsed ? 'agentic-mode-config__tools-section--expanded' : ''}`}>
            <div 
              className="agentic-mode-config__expand-trigger"
              onClick={() => setToolsCollapsed(!toolsCollapsed)}
              role="button"
              tabIndex={0}
            >
              <span className="agentic-mode-config__expand-line" />
              <span className="agentic-mode-config__expand-text">
                {t('tools.available', 'Available Tools')} ({agenticConfig.available_tools?.length || 0}/{availableTools.length})
              </span>
              <span className="agentic-mode-config__expand-line" />
            </div>
            
            {!toolsCollapsed && (
              <div className="agentic-mode-config__expand-content">
                <div className="agentic-mode-config__tools-actions">
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={selectAllTools}
                    disabled={loading}
                    tooltip={t('tools.selectAll')}
                  >
                    <ListChecks size={18} />
                  </IconButton>
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={clearAllTools}
                    disabled={loading}
                    tooltip={t('tools.clear')}
                  >
                    <X size={18} />
                  </IconButton>
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={resetToolsConfig}
                    disabled={loading}
                    tooltip={t('tools.reset')}
                  >
                    <RotateCcw size={18} />
                  </IconButton>
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={saveToolsConfig}
                    disabled={loading}
                    tooltip={t('tools.save')}
                  >
                    <Save size={18} />
                  </IconButton>
                </div>

                <div className="agentic-mode-config__tools-grid">
                  {[...availableTools]
                    .sort((a, b) => {
                      const aSelected = agenticConfig.available_tools?.includes(a.name);
                      const bSelected = agenticConfig.available_tools?.includes(b.name);
                      if (aSelected && !bSelected) return -1;
                      if (!aSelected && bSelected) return 1;
                      return 0;
                    })
                    .map(tool => {
                      const isSelected = agenticConfig.available_tools?.includes(tool.name);
                      return (
                        <Card
                          key={tool.name}
                          variant="default"
                          padding="none"
                          interactive
                          className={`agentic-mode-config__tool-item ${isSelected ? 'agentic-mode-config__tool-item--selected' : ''}`}
                          onClick={() => {
                            if (!loading) {
                              toggleTool(tool.name).catch(err => {
                                log.error('Failed to toggle tool', { toolName: tool.name, error: err });
                              });
                            }
                          }}
                          style={{ cursor: loading ? 'default' : 'pointer' }}
                        >
                          <span className="agentic-mode-config__tool-content">
                            <span className="agentic-mode-config__tool-name">{tool.name}</span>
                            {isSelected && (
                              <span className="agentic-mode-config__tool-badge">{t('tools.enabled')}</span>
                            )}
                          </span>
                        </Card>
                      );
                    })}
                </div>
              </div>
            )}
          </div>
        )}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AgenticModeConfig;

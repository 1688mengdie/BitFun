 

import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle, XCircle } from 'lucide-react';
import { Search, Switch, NumberInput, Card, CardBody, FilterPill, FilterPillGroup } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { toolAPI } from '../../api/service-api/ToolAPI';
import { configManager } from '../services/ConfigManager';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AgenticToolsConfig.scss';

const log = createLogger('AgenticToolsConfig');

interface ToolInfo {
  name: string;
  description: string;
  input_schema: any;
  is_readonly: boolean;
  is_concurrency_safe: boolean;
  needs_permissions: boolean;
}

type FilterType = 'all' | 'readonly' | 'writable';

const AgenticToolsConfig: React.FC = () => {
  const { t } = useTranslation('settings/agentic-tools');
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchKeyword, setSearchKeyword] = useState('');
  const [filterType, setFilterType] = useState<FilterType>('all');
  const [expandedToolNames, setExpandedToolNames] = useState<Set<string>>(new Set());
  
  
  const [skipToolConfirmation, setSkipToolConfirmation] = useState<boolean>(false);
  const [executionTimeout, setExecutionTimeout] = useState<string>(''); 
  const [confirmationTimeout, setConfirmationTimeout] = useState<string>(''); 
  const [configLoading, setConfigLoading] = useState(false);

  
  useEffect(() => {
    loadTools();
    loadToolConfig();
  }, []);

  const loadTools = async () => {
    try {
      setLoading(true);
      setError(null);
      const toolsData = await toolAPI.getAllToolsInfo();
      setTools(toolsData);
    } catch (err) {
      log.error('Failed to load tools', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  
  const loadToolConfig = async () => {
    try {
      const [skipConfirm, execTimeout, confirmTimeout] = await Promise.all([
        configManager.getConfig<boolean>('ai.skip_tool_confirmation'),
        configManager.getConfig<number | null>('ai.tool_execution_timeout_secs'),
        configManager.getConfig<number | null>('ai.tool_confirmation_timeout_secs'),
      ]);
      
      setSkipToolConfirmation(skipConfirm || false);
      setExecutionTimeout(execTimeout != null ? String(execTimeout) : '');
      setConfirmationTimeout(confirmTimeout != null ? String(confirmTimeout) : '');
    } catch (err) {
      log.error('Failed to load config', err);
    }
  };

  
  const handleSkipConfirmationChange = async (checked: boolean) => {
    setSkipToolConfirmation(checked);
    setConfigLoading(true);
    try {
      await configManager.setConfig('ai.skip_tool_confirmation', checked);
      notificationService.success(
        checked ? t('messages.autoExecuteEnabled') : t('messages.autoExecuteDisabled'),
        { duration: 2000 }
      );
      
      
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (err) {
      log.error('Failed to save config', err);
      notificationService.error(`${t('messages.saveFailed')}: ` + (err instanceof Error ? err.message : String(err)));
      setSkipToolConfirmation(!checked); 
    } finally {
      setConfigLoading(false);
    }
  };

  
  const handleTimeoutChange = async (type: 'execution' | 'confirmation', value: string) => {
    const configKey = type === 'execution' 
      ? 'ai.tool_execution_timeout_secs' 
      : 'ai.tool_confirmation_timeout_secs';
    
    
    const trimmedValue = value.trim();
    if (trimmedValue !== '') {
      const numValue = parseInt(trimmedValue, 10);
      if (isNaN(numValue) || numValue < 0) {
        
        return;
      }
    }
    
    
    if (type === 'execution') {
      setExecutionTimeout(trimmedValue);
    } else {
      setConfirmationTimeout(trimmedValue);
    }
    
    
    const numValue = trimmedValue === '' ? null : parseInt(trimmedValue, 10);
    
    try {
      await configManager.setConfig(configKey, numValue);
    } catch (err) {
      log.error('Failed to save timeout config', { type, error: err });
      notificationService.error(t('messages.saveFailed'));
    }
  };

  
  const filteredTools = tools.filter(tool => {
    
    if (filterType === 'readonly' && !tool.is_readonly) {
      return false;
    }
    if (filterType === 'writable' && tool.is_readonly) {
      return false;
    }

    
    if (searchKeyword) {
      const keyword = searchKeyword.toLowerCase();
      return (
        tool.name.toLowerCase().includes(keyword) ||
        tool.description.toLowerCase().includes(keyword)
      );
    }

    return true;
  });

  
  const toggleToolExpanded = (toolName: string) => {
    setExpandedToolNames(prev => {
      const next = new Set(prev);
      if (next.has(toolName)) {
        next.delete(toolName);
      } else {
        next.add(toolName);
      }
      return next;
    });
  };

  
  const formatSchema = (schema: any): string => {
    try {
      return JSON.stringify(schema, null, 2);
    } catch {
      return String(schema);
    }
  };

  
  const renderToolsList = () => {
    if (loading) {
      return <div className="bitfun-agentic-tools__loading">{t('messages.loading')}</div>;
    }

    if (error) {
      return (
        <div className="bitfun-agentic-tools__error">
          {t('messages.loadFailed')}: {error}
          <button 
            className="bitfun-agentic-tools__retry-button"
            onClick={loadTools}
          >
            {t('messages.retry')}
          </button>
        </div>
      );
    }

    if (filteredTools.length === 0) {
      return (
        <div className="bitfun-agentic-tools__empty">
          {searchKeyword ? t('messages.noMatchingTools') : t('messages.noTools')}
        </div>
      );
    }

    return (
      <div className="bitfun-agentic-tools__list">
        {filteredTools.map((tool) => {
          const isExpanded = expandedToolNames.has(tool.name);

          return (
            <Card
              key={tool.name}
              variant="default"
              padding="none"
              className={`bitfun-agentic-tools__item ${isExpanded ? 'is-expanded' : ''}`}
            >
              
              <div
                className="bitfun-agentic-tools__item-header"
                onClick={() => toggleToolExpanded(tool.name)}
              >
                <div className="bitfun-agentic-tools__item-main">
                  <div className="bitfun-agentic-tools__item-info">
                    <div className="bitfun-agentic-tools__item-name">{tool.name}</div>
                    <div className="bitfun-agentic-tools__item-description">{tool.description}</div>
                  </div>
                </div>
                <div className="bitfun-agentic-tools__item-badges">
                  {tool.is_readonly ? (
                    <span className="bitfun-agentic-tools__badge bitfun-agentic-tools__badge--readonly">
                      {t('badges.readonly')}
                    </span>
                  ) : (
                    <span className="bitfun-agentic-tools__badge bitfun-agentic-tools__badge--writable">
                      {t('badges.writable')}
                    </span>
                  )}
                  {tool.is_concurrency_safe && (
                    <span className="bitfun-agentic-tools__badge bitfun-agentic-tools__badge--concurrent">
                      {t('badges.concurrent')}
                    </span>
                  )}
                  {tool.needs_permissions && (
                    <span className="bitfun-agentic-tools__badge bitfun-agentic-tools__badge--permission">
                      {t('badges.permission')}
                    </span>
                  )}
                </div>
              </div>

              
              {isExpanded && (
                <CardBody className="bitfun-agentic-tools__item-details">
                  <div className="bitfun-agentic-tools__detail-section">
                    <div className="bitfun-agentic-tools__detail-label">{t('properties.title')}</div>
                    <div className="bitfun-agentic-tools__properties">
                      <div className="bitfun-agentic-tools__property">
                        <span className="bitfun-agentic-tools__property-label">{t('properties.readonlyMode')}:</span>
                        <span className="bitfun-agentic-tools__property-value">
                          {tool.is_readonly ? (
                            <CheckCircle size={14} className="bitfun-agentic-tools__icon-check" />
                          ) : (
                            <XCircle size={14} className="bitfun-agentic-tools__icon-cross" />
                          )}
                          {tool.is_readonly ? t('properties.yes') : t('properties.no')}
                        </span>
                      </div>
                      <div className="bitfun-agentic-tools__property">
                        <span className="bitfun-agentic-tools__property-label">{t('properties.concurrencySafe')}:</span>
                        <span className="bitfun-agentic-tools__property-value">
                          {tool.is_concurrency_safe ? (
                            <CheckCircle size={14} className="bitfun-agentic-tools__icon-check" />
                          ) : (
                            <XCircle size={14} className="bitfun-agentic-tools__icon-cross" />
                          )}
                          {tool.is_concurrency_safe ? t('properties.yes') : t('properties.no')}
                        </span>
                      </div>
                      <div className="bitfun-agentic-tools__property">
                        <span className="bitfun-agentic-tools__property-label">{t('properties.needsPermission')}:</span>
                        <span className="bitfun-agentic-tools__property-value">
                          {tool.needs_permissions ? (
                            <CheckCircle size={14} className="bitfun-agentic-tools__icon-check" />
                          ) : (
                            <XCircle size={14} className="bitfun-agentic-tools__icon-cross" />
                          )}
                          {tool.needs_permissions ? t('properties.yes') : t('properties.no')}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="bitfun-agentic-tools__detail-section">
                    <div className="bitfun-agentic-tools__detail-label">{t('schema.title')}</div>
                    <pre className="bitfun-agentic-tools__schema">
                      {formatSchema(tool.input_schema)}
                    </pre>
                  </div>
                </CardBody>
              )}
            </Card>
          );
        })}
      </div>
    );
  };

  
  const stats = {
    total: tools.length,
    readonly: tools.filter(t => t.is_readonly).length,
    writable: tools.filter(t => !t.is_readonly).length,
  };

  return (
    <ConfigPageLayout className="bitfun-agentic-tools">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="bitfun-agentic-tools__content">
        
        <div className="bitfun-agentic-tools__global-config">
          <div className="bitfun-agentic-tools__config-row">
            <Switch
              checked={skipToolConfirmation}
              onChange={(e) => handleSkipConfirmationChange(e.target.checked)}
              disabled={configLoading}
              label={t('config.autoExecute')}
              size="small"
            />
          </div>
          <div className="bitfun-agentic-tools__config-row bitfun-agentic-tools__timeout-row">
            <div className="bitfun-agentic-tools__timeout-item">
              <NumberInput
                label={t('config.confirmTimeout')}
                value={confirmationTimeout === '' ? 0 : parseInt(confirmationTimeout, 10)}
                onChange={(val) => handleTimeoutChange('confirmation', val === 0 ? '' : String(val))}
                min={0}
                max={3600}
                step={5}
                unit={t('config.seconds')}
                size="small"
                variant="compact"
              />
            </div>
            <div className="bitfun-agentic-tools__timeout-item">
              <NumberInput
                label={t('config.executionTimeout')}
                value={executionTimeout === '' ? 0 : parseInt(executionTimeout, 10)}
                onChange={(val) => handleTimeoutChange('execution', val === 0 ? '' : String(val))}
                min={0}
                max={3600}
                step={5}
                unit={t('config.seconds')}
                size="small"
                variant="compact"
              />
            </div>
          </div>
        </div>

        
        <div className="bitfun-agentic-tools__fixed-header">
          
          <div className="bitfun-agentic-tools__toolbar">
            <div className="bitfun-agentic-tools__search-box">
              <Search
                placeholder={t('search.placeholder')}
                value={searchKeyword}
                onChange={(val) => setSearchKeyword(val)}
                clearable
                size="small"
              />
            </div>
          </div>
          
          <FilterPillGroup className="bitfun-agentic-tools__filters">
            <FilterPill
              label={t('filters.all')}
              count={stats.total}
              active={filterType === 'all'}
              onClick={() => setFilterType('all')}
            />
            <FilterPill
              label={t('filters.readonly')}
              count={stats.readonly}
              active={filterType === 'readonly'}
              onClick={() => setFilterType('readonly')}
            />
            <FilterPill
              label={t('filters.writable')}
              count={stats.writable}
              active={filterType === 'writable'}
              onClick={() => setFilterType('writable')}
            />
          </FilterPillGroup>
        </div>

        
        <div className="bitfun-agentic-tools__scrollable-area">
          {renderToolsList()}
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AgenticToolsConfig;


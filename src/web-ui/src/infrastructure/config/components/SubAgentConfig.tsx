 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch, Search, FilterPill, FilterPillGroup, Card, CardBody, Tooltip, Button, IconButton, Input, Textarea, Checkbox } from '@/component-library';
import { Trash2, Plus, RefreshCw } from 'lucide-react';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { ModelSelectionRadio } from './ModelSelectionRadio';
import { useCurrentWorkspace } from '../../hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { SubagentAPI, type SubagentInfo, type SubagentSource, type SubagentLevel } from '../../api/service-api/SubagentAPI';
import { configAPI } from '../../api/service-api/ConfigAPI';
import { configManager } from '../services/ConfigManager';
import type { AIModelConfig } from '../types';
import './SubAgentConfig.scss';

type FilterLevel = 'all' | 'user' | 'project' | 'builtin';

function filterBySearch(agents: SubagentInfo[], keyword: string): SubagentInfo[] {
  if (!keyword) return agents;
  const k = keyword.toLowerCase();
  return agents.filter(
    (a) =>
      a.name.toLowerCase().includes(k) ||
      a.description.toLowerCase().includes(k) ||
      a.defaultTools.some((t) => t.toLowerCase().includes(k))
  );
}

function getBadgeLabel(source: SubagentSource | undefined, t: (key: string) => string): string {
  switch (source) {
    case 'builtin':
      return t('list.item.builtin');
    case 'user':
      return t('filters.user');
    case 'project':
      return t('filters.project');
    default:
      return '';
  }
}

const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

const SubAgentConfig: React.FC = () => {
  const { t } = useTranslation('settings/agents');
  const [filterLevel, setFilterLevel] = useState<FilterLevel>('all');
  const [searchKeyword, setSearchKeyword] = useState('');
  const [expandedAgentIds, setExpandedAgentIds] = useState<Set<string>>(new Set());

  const [allSubagents, setAllSubagents] = useState<SubagentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [availableModels, setAvailableModels] = useState<AIModelConfig[]>([]);
  const [agentModels, setAgentModels] = useState<Record<string, string>>({});

  const [showAddForm, setShowAddForm] = useState(false);
  const [toolNames, setToolNames] = useState<string[]>([]);
  const [formLevel, setFormLevel] = useState<SubagentLevel>('user');
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formPrompt, setFormPrompt] = useState('');
  const [formReadonly, setFormReadonly] = useState(true);
  const [formSelectedTools, setFormSelectedTools] = useState<Set<string>>(new Set());
  const [formNameError, setFormNameError] = useState<string | null>(null);
  const [formSubmitting, setFormSubmitting] = useState(false);

  const { workspacePath, hasWorkspace } = useCurrentWorkspace();
  const notification = useNotification();

  const refreshAllSubagents = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await SubagentAPI.listSubagents();
      setAllSubagents(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const [agents, allModels, agentModelsData] = await Promise.all([
          SubagentAPI.listSubagents(),
          configManager.getConfig<AIModelConfig[]>('ai.models') || [],
          configManager.getConfig<Record<string, string>>('ai.agent_models') || {},
        ]);
        setAllSubagents(agents);
        setAvailableModels(allModels);
        setAgentModels(agentModelsData);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  useEffect(() => {
    if (hasWorkspace && workspacePath) {
      refreshAllSubagents();
    }
  }, [hasWorkspace, workspacePath, refreshAllSubagents]);

  useEffect(() => {
    if (showAddForm && toolNames.length === 0) {
      SubagentAPI.listAgentToolNames().then(setToolNames).catch(() => setToolNames([]));
    }
  }, [showAddForm, toolNames.length]);

  const validateName = useCallback((name: string): string | null => {
    const s = name.trim();
    if (!s) return t('messages.nameRequired');
    if (!NAME_REGEX.test(s)) return t('messages.nameFormatError');
    return null;
  }, [t]);

  const handleFormNameChange = useCallback((val: string) => {
    setFormName(val);
    setFormNameError(validateName(val) || null);
  }, [validateName]);

  const handleFormNameBlur = useCallback(() => {
    if (formName.trim()) setFormNameError(validateName(formName) || null);
  }, [formName, validateName]);

  const toggleFormTool = useCallback((tool: string) => {
    setFormSelectedTools(prev => {
      const next = new Set(prev);
      if (next.has(tool)) next.delete(tool);
      else next.add(tool);
      return next;
    });
  }, []);

  const resetAddForm = useCallback(() => {
    setFormLevel('user');
    setFormName('');
    setFormDescription('');
    setFormPrompt('');
    setFormReadonly(true);
    setFormSelectedTools(new Set());
    setFormNameError(null);
    setShowAddForm(false);
  }, []);

  const handleCreateSubmit = useCallback(async () => {
    const name = formName.trim();
    const desc = formDescription.trim();
    const prompt = formPrompt.trim();
    const nameErr = validateName(name);
    if (nameErr) {
      setFormNameError(nameErr);
      return;
    }
    if (!desc) {
      notification.error(t('messages.descriptionRequired'));
      return;
    }
    if (!prompt) {
      notification.error(t('messages.contentRequired'));
      return;
    }
    setFormSubmitting(true);
    try {
      await SubagentAPI.createSubagent({
        level: formLevel,
        name,
        description: desc,
        prompt,
        readonly: formReadonly,
        tools: formSelectedTools.size > 0 ? Array.from(formSelectedTools) : undefined,
      });
      notification.success(t('messages.createSuccess', { name }));
      resetAddForm();
      await refreshAllSubagents();
    } catch (err) {
      notification.error(t('messages.operationFailed', { operation: t('messages.create'), error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setFormSubmitting(false);
    }
  }, [formLevel, formName, formDescription, formPrompt, formReadonly, formSelectedTools, validateName, notification, t, resetAddForm, refreshAllSubagents]);

  const bySource = React.useMemo(() => {
    const builtin = allSubagents.filter((a) => a.subagentSource === 'builtin');
    const user = allSubagents.filter((a) => a.subagentSource === 'user');
    const project = allSubagents.filter((a) => a.subagentSource === 'project');
    return { builtin, user, project };
  }, [allSubagents]);

  const filteredList = React.useMemo(() => {
    let list: SubagentInfo[];
    if (filterLevel === 'all') {
      list = [...bySource.builtin, ...bySource.user, ...bySource.project];
    } else {
      list = bySource[filterLevel];
    }
    return filterBySearch(list, searchKeyword);
  }, [filterLevel, searchKeyword, bySource]);

  
  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    return availableModels.find(m => m.id === modelId)?.name;
  }, [availableModels]);

  
  const enabledModels = availableModels.filter(m => m.enabled);

  const handleAgentModelChange = async (agent: SubagentInfo, modelId: string) => {
    try {
      const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';

      if (isCustom) {
        
        await SubagentAPI.updateSubagentConfig({
          subagentId: agent.id,
          model: modelId,
        });
        
        await refreshAllSubagents();
      } else {
        
        const currentAgentModels = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};
        const updatedAgentModels = { ...currentAgentModels, [agent.id]: modelId };
        await configManager.setConfig('ai.agent_models', updatedAgentModels);
        setAgentModels(updatedAgentModels);
      }

      let modelName = modelId;
      if (modelId === 'primary') modelName = t('model.primary');
      else if (modelId === 'fast') modelName = t('model.fast');
      else modelName = getModelName(modelId) || modelId;
      notification.success(t('messages.modelUpdated', { name: agent.name, model: modelName }), { duration: 2000 });
    } catch (err) {
      notification.error(t('messages.modelUpdateFailed'));
    }
  };

  const handleToggle = useCallback(
    async (agent: SubagentInfo) => {
      const newEnabled = !agent.enabled;
      const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';
      try {
        if (isCustom) {
          
          await SubagentAPI.updateSubagentConfig({
            subagentId: agent.id,
            enabled: newEnabled,
          });
        } else {
          
          await configAPI.setSubagentConfig(agent.id, newEnabled);
        }
        await refreshAllSubagents();
        notification.success(t('messages.toggleSuccess', { name: agent.name, status: newEnabled ? t('messages.enabled') : t('messages.disabled') }));
      } catch (err) {
        notification.error(t('messages.toggleFailed', { error: err instanceof Error ? err.message : String(err) }));
      }
    },
    [refreshAllSubagents, notification, t]
  );

  const handleDelete = useCallback(
    async (agent: SubagentInfo) => {
      const confirmed = await window.confirm(t('messages.confirmDeleteSubagent', { name: agent.name }));
      if (!confirmed) return;
      try {
        await configAPI.deleteSubagent(agent.id);
        await refreshAllSubagents();
        notification.success(t('messages.deleteSuccess', { name: agent.name }));
      } catch (err) {
        notification.error(t('messages.deleteFailed', { error: err instanceof Error ? err.message : String(err) }));
      }
    },
    [refreshAllSubagents, notification, t]
  );

  
  const toggleAgentExpanded = (agentId: string) => {
    setExpandedAgentIds(prev => {
      const next = new Set(prev);
      if (next.has(agentId)) {
        next.delete(agentId);
      } else {
        next.add(agentId);
      }
      return next;
    });
  };

   
  const getAgentModel = useCallback((agent: SubagentInfo): string => {
    const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';
    if (isCustom) {
      
      return agent.model || 'primary';
    } else {
      
      return agentModels[agent.id] || 'primary';
    }
  }, [agentModels]);

   
  const renderAgentDetails = (agent: SubagentInfo) => (
    <CardBody className="bitfun-sub-agent-config__item-details">
      <div className="bitfun-sub-agent-config__item-description">{agent.description}</div>
      <div className="bitfun-sub-agent-config__model-config">
        <div className="bitfun-sub-agent-config__config-row">
          <div className="bitfun-sub-agent-config__config-label">
            {t('list.item.modelLabel')}
          </div>
          <div className="bitfun-sub-agent-config__config-control">
            <ModelSelectionRadio
              value={getAgentModel(agent)}
              models={enabledModels}
              onChange={(modelId) => handleAgentModelChange(agent, modelId)}
              layout="horizontal"
              size="small"
            />
          </div>
        </div>
      </div>
      <div className="bitfun-sub-agent-config__item-tools">
        <div className="bitfun-sub-agent-config__tools-label">{t('list.item.toolsLabel')}</div>
        <div className="bitfun-sub-agent-config__tools-list">
          {agent.defaultTools.map((tool, idx) => (
            <span key={idx} className="bitfun-sub-agent-config__tool-tag">
              {tool}
            </span>
          ))}
        </div>
      </div>
    </CardBody>
  );

  const renderAgentCard = (agent: SubagentInfo) => {
    const isExpanded = expandedAgentIds.has(agent.id);
    const toolCount = agent.toolCount ?? agent.defaultTools.length;
    const badgeLabel = getBadgeLabel(agent.subagentSource, t);
    const isBuiltin = agent.subagentSource === 'builtin';
    return (
      <Card
        key={agent.id}
        variant="default"
        padding="none"
        interactive
        className={`bitfun-sub-agent-config__item ${isBuiltin ? 'bitfun-sub-agent-config__item--builtin' : ''} ${isExpanded ? 'is-expanded' : ''} ${!agent.enabled ? 'is-disabled' : ''}`}
      >
        <div
          className="bitfun-sub-agent-config__item-header"
          onClick={() => toggleAgentExpanded(agent.id)}
        >
          <div className="bitfun-sub-agent-config__item-main">
            <div className="bitfun-sub-agent-config__item-name">{agent.name}</div>
            <div className="bitfun-sub-agent-config__item-badges">
              <span className="bitfun-sub-agent-config__badge bitfun-sub-agent-config__badge--builtin">
                {badgeLabel}
              </span>
              <span className="bitfun-sub-agent-config__badge bitfun-sub-agent-config__badge--tools">
                {t('list.item.toolsCount', { count: toolCount })}
              </span>
            </div>
          </div>
          <div className="bitfun-sub-agent-config__item-actions" onClick={(e) => e.stopPropagation()}>
            <Switch
              checked={agent.enabled}
              onChange={(e) => { e.stopPropagation(); handleToggle(agent); }}
              size="small"
            />
            {!isBuiltin && (
              <Tooltip content={t('list.item.deleteTooltip')}>
                <button
                  type="button"
                  className="bitfun-sub-agent-config__action-button bitfun-sub-agent-config__action-button--delete"
                  onClick={(e) => { e.stopPropagation(); handleDelete(agent); }}
                  aria-label={t('list.item.deleteTooltip')}
                >
                  <Trash2 size={14} />
                </button>
              </Tooltip>
            )}
          </div>
        </div>
        {isExpanded && renderAgentDetails(agent)}
      </Card>
    );
  };

  const renderList = () => {
    if (loading) return <div className="bitfun-sub-agent-config__loading">{t('list.loading')}</div>;
    if (error) return <div className="bitfun-sub-agent-config__error">{t('list.errorPrefix')}{error}</div>;
    if (filteredList.length === 0) {
      return (
        <div className="bitfun-sub-agent-config__empty">
          {searchKeyword ? t('list.empty.noMatch') : t('list.empty.noAgents')}
        </div>
      );
    }
    return (
      <div className="bitfun-sub-agent-config__list">
        {filteredList.map((agent) => renderAgentCard(agent))}
      </div>
    );
  };

  return (
    <ConfigPageLayout className="bitfun-sub-agent-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="bitfun-sub-agent-config__content">
        
        <div className="bitfun-sub-agent-config__section">
          <div className="bitfun-sub-agent-config__toolbar">
            <div className="bitfun-sub-agent-config__search-box">
              <Search
                placeholder={t('toolbar.searchPlaceholder')}
                value={searchKeyword}
                onChange={(val) => setSearchKeyword(val)}
                clearable
                size="small"
              />
            </div>
            <IconButton
              variant="default"
              size="small"
              onClick={async () => {
                try {
                  await SubagentAPI.reloadSubagents();
                  await refreshAllSubagents();
                  notification.success(t('toolbar.refreshSuccess'));
                } catch (err) {
                  notification.error(err instanceof Error ? err.message : String(err));
                }
              }}
              tooltip={t('toolbar.refreshTooltip')}
            >
              <RefreshCw size={16} />
            </IconButton>
            <IconButton
              variant="primary"
              size="small"
              onClick={() => setShowAddForm(true)}
              tooltip={t('toolbar.addTooltip')}
            >
              <Plus size={16} />
            </IconButton>
          </div>

          {showAddForm && (
            <div className="bitfun-sub-agent-config__form">
              <div className="bitfun-sub-agent-config__form-header">
                <h3>{t('form.titleCreate')}</h3>
              </div>
              <div className="bitfun-sub-agent-config__form-body">
                <div className="bitfun-sub-agent-config__form-group">
                  <label>{t('form.fields.level')}</label>
                  <div className="bitfun-sub-agent-config__form-level">
                    <label className="bitfun-sub-agent-config__form-radio">
                      <input
                        type="radio"
                        name="level"
                        checked={formLevel === 'user'}
                        onChange={() => setFormLevel('user')}
                      />
                      {t('form.fields.levelUser')}
                    </label>
                    <label className="bitfun-sub-agent-config__form-radio">
                      <input
                        type="radio"
                        name="level"
                        checked={formLevel === 'project'}
                        onChange={() => setFormLevel('project')}
                        disabled={!hasWorkspace}
                      />
                      {t('form.fields.levelProject')}
                      {!hasWorkspace && t('form.fields.levelProjectDisabled')}
                    </label>
                  </div>
                </div>
                <div className="bitfun-sub-agent-config__form-group">
                  <label>{t('form.fields.name')}</label>
                  <Input
                    value={formName}
                    onChange={(e) => handleFormNameChange(e.target.value)}
                    onBlur={handleFormNameBlur}
                    placeholder={t('form.fields.namePlaceholder')}
                    inputSize="small"
                    error={!!formNameError}
                  />
                  {formNameError && (
                    <span className="bitfun-sub-agent-config__form-error">{formNameError}</span>
                  )}
                </div>
                <div className="bitfun-sub-agent-config__form-group">
                  <label>{t('form.fields.description')}</label>
                  <Input
                    value={formDescription}
                    onChange={(e) => setFormDescription(e.target.value)}
                    placeholder={t('form.fields.descriptionPlaceholder')}
                    inputSize="small"
                  />
                </div>
                <div className="bitfun-sub-agent-config__form-group">
                  <label>{t('form.fields.systemPrompt')}</label>
                  <Textarea
                    value={formPrompt}
                    onChange={(e) => setFormPrompt(e.target.value)}
                    placeholder={t('form.fields.systemPromptPlaceholder')}
                    rows={6}
                    className="bitfun-sub-agent-config__form-textarea"
                  />
                </div>
                <div className="bitfun-sub-agent-config__form-group">
                  <div className="bitfun-sub-agent-config__form-row bitfun-sub-agent-config__form-row--switch">
                    <label>{t('form.fields.readonly')}</label>
                    <Switch
                      checked={formReadonly}
                      onChange={(e) => setFormReadonly(e.target.checked)}
                      size="small"
                    />
                  </div>
                  {formReadonly && (
                    <div className="bitfun-sub-agent-config__form-hint">{t('form.fields.readonlyDescription')}</div>
                  )}
                </div>
                <div className="bitfun-sub-agent-config__form-group">
                  <label>{t('form.fields.tools')} ({t('form.fields.toolsOptional')})</label>
                  <div className="bitfun-sub-agent-config__form-tools">
                    {toolNames.map((tool) => (
                      <label key={tool} className="bitfun-sub-agent-config__form-tool-check">
                        <Checkbox
                          checked={formSelectedTools.has(tool)}
                          onChange={() => toggleFormTool(tool)}
                        />
                        <span>{tool}</span>
                      </label>
                    ))}
                  </div>
                </div>
              </div>
              <div className="bitfun-sub-agent-config__form-footer">
                <Button variant="secondary" size="small" onClick={resetAddForm} disabled={formSubmitting}>
                  {t('form.actions.cancel')}
                </Button>
                <Button variant="primary" size="small" onClick={handleCreateSubmit} disabled={formSubmitting}>
                  {formSubmitting ? '...' : t('form.actions.create')}
                </Button>
              </div>
            </div>
          )}

          
          <FilterPillGroup className="bitfun-sub-agent-config__filters">
            <FilterPill
              label={t('filters.all')}
              count={`${allSubagents.filter((a) => a.enabled).length}/${allSubagents.length}`}
              active={filterLevel === 'all'}
              onClick={() => setFilterLevel('all')}
            />
            <FilterPill
              label={t('filters.builtin')}
              count={`${bySource.builtin.filter((a) => a.enabled).length}/${bySource.builtin.length}`}
              active={filterLevel === 'builtin'}
              onClick={() => setFilterLevel('builtin')}
            />
            <FilterPill
              label={t('filters.user')}
              count={`${bySource.user.filter((a) => a.enabled).length}/${bySource.user.length}`}
              active={filterLevel === 'user'}
              onClick={() => setFilterLevel('user')}
            />
            <FilterPill
              label={t('filters.project')}
              count={`${bySource.project.filter((a) => a.enabled).length}/${bySource.project.length}`}
              active={filterLevel === 'project'}
              onClick={() => setFilterLevel('project')}
            />
          </FilterPillGroup>
        </div>

        
        {renderList()}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default SubAgentConfig;

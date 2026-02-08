 

import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Edit2, Trash2, FileText, X, ChevronDown, ChevronRight, RefreshCw } from 'lucide-react';
import { Select, Input, Textarea, Button, Search, IconButton, Switch, Tooltip, Card, CardBody, FilterPill, FilterPillGroup } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { useAIRules } from '../../hooks/useAIRules';
import {
  AIRulesAPI,
  RuleLevel,
  RuleApplyType,
  type CreateRuleRequest,
  type AIRule
} from '../../api/service-api/AIRulesAPI';
import { createLogger } from '@/shared/utils/logger';
import './AIRulesConfig.scss';

const log = createLogger('AIRulesConfig');

type FilterLevel = 'all' | 'user' | 'project';

const AIRulesConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-rules');
  const [filterLevel, setFilterLevel] = useState<FilterLevel>('all');
  const [searchKeyword, setSearchKeyword] = useState('');
  const [showAddForm, setShowAddForm] = useState(false);
  const [editingRule, setEditingRule] = useState<AIRule | null>(null);
  const [expandedRuleNames, setExpandedRuleNames] = useState<Set<string>>(new Set());
  const [isDeleting, setIsDeleting] = useState(false);

  
  const [formLevel, setFormLevel] = useState<RuleLevel>(RuleLevel.User);
  const [formData, setFormData] = useState<CreateRuleRequest>({
    name: '',
    apply_type: RuleApplyType.AlwaysApply,
    content: '',
    description: '',
    globs: '',
  });

  
  const userRules = useAIRules(RuleLevel.User);
  const projectRules = useAIRules(RuleLevel.Project);

  
  const allRules = {
    rules: [
      ...userRules.rules,
      ...projectRules.rules,
    ],
    stats: {
      total_rules: (userRules.stats?.total_rules || 0) + (projectRules.stats?.total_rules || 0),
      enabled_rules: (userRules.stats?.enabled_rules || 0) + (projectRules.stats?.enabled_rules || 0),
      by_apply_type: { ...userRules.stats?.by_apply_type, ...projectRules.stats?.by_apply_type },
    }
  };

  
  const filteredRules = allRules.rules.filter(rule => {
    
    if (filterLevel === 'user' && rule.level !== 'user') return false;
    if (filterLevel === 'project' && rule.level !== 'project') return false;

    
    if (searchKeyword) {
      const keyword = searchKeyword.toLowerCase();
      return (
        rule.name.toLowerCase().includes(keyword) ||
        rule.description?.toLowerCase().includes(keyword) ||
        rule.content.toLowerCase().includes(keyword)
      );
    }

    return true;
  });

  
  const handleSubmit = async () => {
    
    if (!formData.name || !formData.name.trim()) {
      alert(t('messages.nameRequired'));
      return;
    }

    if (!formData.content || !formData.content.trim()) {
      alert(t('messages.contentRequired'));
      return;
    }

    
    const finalApplyType = formLevel === RuleLevel.User 
      ? RuleApplyType.AlwaysApply 
      : formData.apply_type;

    
    const ruleData: CreateRuleRequest = {
      name: formData.name.trim(),
      apply_type: finalApplyType,
      content: formData.content,
    };

    if (finalApplyType === RuleApplyType.ApplyIntelligently && formData.description) {
      ruleData.description = formData.description;
    }
    if (finalApplyType === RuleApplyType.ApplyToSpecificFiles && formData.globs) {
      ruleData.globs = formData.globs;
    }

    try {
      if (editingRule) {
        
        await AIRulesAPI.updateRule(formLevel, editingRule.name, {
          name: ruleData.name !== editingRule.name ? ruleData.name : undefined,
          apply_type: ruleData.apply_type,
          content: ruleData.content,
          description: ruleData.description,
          globs: ruleData.globs,
        });
        
        
        if (formLevel === RuleLevel.User) {
          await userRules.refresh();
        } else {
          await projectRules.refresh();
        }
      } else {
        
        if (formLevel === RuleLevel.User) {
          await userRules.createRule(ruleData);
        } else {
          await projectRules.createRule(ruleData);
        }
      }

      
      resetForm();
    } catch (error) {
      log.error('Failed to save rule', error);
      alert(t('messages.saveFailed', { error: error instanceof Error ? error.message : String(error) }));
    }
  };

  const handleAdd = () => {
    resetForm();
    setShowAddForm(true);
    setEditingRule(null);
  };

  const handleEdit = (rule: AIRule) => {
    setFormData({
      name: rule.name,
      apply_type: rule.apply_type as RuleApplyType,
      content: rule.content,
      description: rule.description || '',
      globs: rule.globs || '',
    });
    setFormLevel(rule.level === 'user' ? RuleLevel.User : RuleLevel.Project);
    setEditingRule(rule);
    setShowAddForm(true);
  };

  const handleDelete = async (rule: AIRule, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    
    if (isDeleting) return;
    
    const confirmed = await window.confirm(t('messages.confirmDelete', { name: rule.name }));
    
    if (!confirmed) return;

    try {
      setIsDeleting(true);
      
      const level = rule.level === 'user' ? RuleLevel.User : RuleLevel.Project;
      await AIRulesAPI.deleteRule(level, rule.name);
      
      
      if (level === RuleLevel.User) {
        await userRules.refresh();
      } else {
        await projectRules.refresh();
      }
    } catch (error) {
      log.error('Failed to delete rule', { ruleName: rule.name, level: rule.level, error });
      alert(t('messages.deleteFailed', { error: error instanceof Error ? error.message : String(error) }));
    } finally {
      setIsDeleting(false);
    }
  };

  const resetForm = () => {
    setFormData({
      name: '',
      apply_type: RuleApplyType.AlwaysApply,
      content: '',
      description: '',
      globs: '',
    });
    setFormLevel(RuleLevel.User);
    setShowAddForm(false);
    setEditingRule(null);
  };

  
  const handleToggle = async (rule: AIRule, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();

    try {
      const level = rule.level === 'user' ? RuleLevel.User : RuleLevel.Project;
      if (level === RuleLevel.User) {
        await userRules.toggleRule(rule.name);
      } else {
        await projectRules.toggleRule(rule.name);
      }
    } catch (error) {
      log.error('Failed to toggle rule', { ruleName: rule.name, level: rule.level, error });
      alert(t('messages.toggleFailed', { error: error instanceof Error ? error.message : String(error) }));
    }
  };

  
  const handleRefresh = async () => {
    try {
      await Promise.all([
        userRules.refresh(),
        projectRules.refresh(),
      ]);
    } catch (error) {
      log.error('Failed to refresh rules', error);
    }
  };

  
  const toggleRuleExpanded = (ruleName: string) => {
    setExpandedRuleNames(prev => {
      const next = new Set(prev);
      if (next.has(ruleName)) {
        next.delete(ruleName);
      } else {
        next.add(ruleName);
      }
      return next;
    });
  };

  
  const getApplyTypeOptions = () => [
    { label: t('form.fields.applyTypes.alwaysApply'), value: RuleApplyType.AlwaysApply },
    { label: t('form.fields.applyTypes.applyIntelligently'), value: RuleApplyType.ApplyIntelligently },
    { label: t('form.fields.applyTypes.applyToSpecificFiles'), value: RuleApplyType.ApplyToSpecificFiles },
    { label: t('form.fields.applyTypes.applyManually'), value: RuleApplyType.ApplyManually },
  ];

  
  const renderForm = () => {
    if (!showAddForm) return null;

    const isUserLevel = formLevel === RuleLevel.User;
    const showDescription = !isUserLevel && formData.apply_type === RuleApplyType.ApplyIntelligently;
    const showGlobs = !isUserLevel && formData.apply_type === RuleApplyType.ApplyToSpecificFiles;

    return (
      <div className="bitfun-ai-rules-config__form">
        <div className="bitfun-ai-rules-config__form-header">
          <h3>{editingRule ? t('form.titleEdit') : t('form.titleCreate')}</h3>
          <IconButton variant="ghost" size="small" onClick={resetForm} tooltip={t('form.closeTooltip')}>
            <X size={14} />
          </IconButton>
        </div>

        <div className="bitfun-ai-rules-config__form-body">
          
          <div className="bitfun-ai-rules-config__form-row">
            <Input
              label={t('form.fields.name')}
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              placeholder={t('form.fields.namePlaceholder')}
              variant="outlined"
              size="small"
            />
            <Select
              label={t('form.fields.level')}
              options={[
                { label: t('form.fields.levelUser'), value: RuleLevel.User },
                { label: t('form.fields.levelProject'), value: RuleLevel.Project }
              ]}
              value={formLevel}
              onChange={(value) => {
                setFormLevel(value as RuleLevel);
                
                if (value === RuleLevel.User) {
                  setFormData({ ...formData, apply_type: RuleApplyType.AlwaysApply });
                }
              }}
              size="medium"
              disabled={!!editingRule} 
            />
          </div>

          
          {!isUserLevel && (
            <Select
              label={t('form.fields.applyType')}
              options={getApplyTypeOptions()}
              value={formData.apply_type}
              onChange={(value) => setFormData({ ...formData, apply_type: value as RuleApplyType })}
              size="medium"
            />
          )}

          
          {showDescription && (
            <Input
              label={t('form.fields.description')}
              value={formData.description || ''}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              placeholder={t('form.fields.descriptionPlaceholder')}
              variant="outlined"
              size="small"
            />
          )}

          
          {showGlobs && (
            <Input
              label={t('form.fields.globs')}
              value={formData.globs || ''}
              onChange={(e) => setFormData({ ...formData, globs: e.target.value })}
              placeholder={t('form.fields.globsPlaceholder')}
              variant="outlined"
              size="small"
            />
          )}

          
          <Textarea
            label={t('form.fields.content')}
            value={formData.content}
            onChange={(e) => setFormData({ ...formData, content: e.target.value })}
            placeholder={t('form.fields.contentPlaceholder')}
            rows={6}
            variant="outlined"
          />
        </div>

        <div className="bitfun-ai-rules-config__form-footer">
          <Button variant="secondary" size="small" onClick={resetForm}>
            {t('form.actions.cancel')}
          </Button>
          <Button variant="primary" size="small" onClick={handleSubmit}>
            {editingRule ? t('form.actions.save') : t('form.actions.add')}
          </Button>
        </div>
      </div>
    );
  };

  
  const renderRulesList = () => {
    if (userRules.isLoading || projectRules.isLoading) {
      return (
        <div className="bitfun-ai-rules-config__empty-state">
          <p>{t('list.loading')}</p>
        </div>
      );
    }

    if (filteredRules.length === 0) {
      return (
        <div className="bitfun-ai-rules-config__empty-state">
          <h3>{t('list.empty.title')}</h3>
          <p>{t('list.empty.description')}</p>
        </div>
      );
    }

    return (
      <div className="bitfun-ai-rules-config__list">
        {filteredRules.map((rule) => {
          const isExpanded = expandedRuleNames.has(rule.name);
          const ruleKey = `${rule.level}-${rule.name}`;
          
          return (
            <Card
              key={ruleKey}
              variant="default"
              padding="none"
              className={`bitfun-ai-rules-config__item ${isExpanded ? 'is-expanded' : ''} ${!rule.enabled ? 'is-disabled' : ''}`}
            >
              
              <div 
                className="bitfun-ai-rules-config__item-header"
                onClick={() => toggleRuleExpanded(rule.name)}
              >
                <div className="bitfun-ai-rules-config__item-expand">
                  {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                </div>
                <div className="bitfun-ai-rules-config__item-main">
                  <div className="bitfun-ai-rules-config__item-name">{rule.name}</div>
                  <div className="bitfun-ai-rules-config__item-badges">
                    <span className={`bitfun-ai-rules-config__badge bitfun-ai-rules-config__badge--level bitfun-ai-rules-config__badge--${rule.level}`}>
                      {rule.level === 'user' ? t('list.item.user') : t('list.item.project')}
                    </span>
                    <span className="bitfun-ai-rules-config__badge bitfun-ai-rules-config__badge--type">
                      {AIRulesAPI.getApplyTypeLabel(rule.apply_type as RuleApplyType)}
                    </span>
                  </div>
                </div>
                <div className="bitfun-ai-rules-config__item-actions" onClick={(e) => e.stopPropagation()}>
                  <Switch
                    checked={rule.enabled}
                    onChange={(e) => handleToggle(rule, e as unknown as React.MouseEvent)}
                    size="small"
                  />
                  <Tooltip content={t('list.item.editTooltip')}>
                    <button
                      className="bitfun-ai-rules-config__action-button"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleEdit(rule);
                      }}
                    >
                      <Edit2 size={14} />
                    </button>
                  </Tooltip>
                  <Tooltip content={t('list.item.deleteTooltip')}>
                    <button
                      className="bitfun-ai-rules-config__action-button bitfun-ai-rules-config__action-button--delete"
                      onClick={(e) => handleDelete(rule, e)}
                    >
                      <Trash2 size={14} />
                    </button>
                  </Tooltip>
                </div>
              </div>

              
              {isExpanded && (
                <CardBody className="bitfun-ai-rules-config__item-details">
                  {rule.description && (
                    <div className="bitfun-ai-rules-config__item-description">
                      <span className="bitfun-ai-rules-config__label">{t('list.item.descriptionLabel')}</span>
                      {rule.description}
                    </div>
                  )}

                  {rule.globs && (
                    <div className="bitfun-ai-rules-config__item-globs">
                      <span className="bitfun-ai-rules-config__label">{t('list.item.globsLabel')}</span>
                      <code>{rule.globs}</code>
                    </div>
                  )}

                  <div className="bitfun-ai-rules-config__item-content">
                    <div className="bitfun-ai-rules-config__content-label">{t('list.item.contentLabel')}</div>
                    <div className="bitfun-ai-rules-config__content-text">{rule.content}</div>
                  </div>

                  <div className="bitfun-ai-rules-config__item-meta">
                    <span>{t('list.item.filePathPrefix')}{rule.file_path}</span>
                  </div>
                </CardBody>
              )}
            </Card>
          );
        })}
      </div>
    );
  };

  return (
    <ConfigPageLayout className="bitfun-ai-rules-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="bitfun-ai-rules-config__content">
        
        <div className="bitfun-ai-rules-config__toolbar">
          <div className="bitfun-ai-rules-config__search-box">
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
            onClick={handleRefresh}
            tooltip={t('toolbar.refreshTooltip')}
          >
            <RefreshCw size={16} />
          </IconButton>
          <IconButton 
            variant="primary" 
            size="small"
            onClick={handleAdd} 
            tooltip={t('toolbar.addTooltip')}
          >
            <Plus size={16} />
          </IconButton>
        </div>

        
        {renderForm()}

        
        <FilterPillGroup className="bitfun-ai-rules-config__filters">
          <FilterPill
            label={t('filters.all')}
            count={`${allRules.stats.enabled_rules}/${allRules.stats.total_rules}`}
            active={filterLevel === 'all'}
            onClick={() => setFilterLevel('all')}
          />
          <FilterPill
            label={t('filters.user')}
            count={`${userRules.stats?.enabled_rules || 0}/${userRules.stats?.total_rules || 0}`}
            active={filterLevel === 'user'}
            onClick={() => setFilterLevel('user')}
          />
          <FilterPill
            label={t('filters.project')}
            count={`${projectRules.stats?.enabled_rules || 0}/${projectRules.stats?.total_rules || 0}`}
            active={filterLevel === 'project'}
            onClick={() => setFilterLevel('project')}
          />
        </FilterPillGroup>

        
        {renderRulesList()}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AIRulesConfig;

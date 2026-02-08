 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, RefreshCw, ChevronDown } from 'lucide-react';
import { Button, Alert, NumberInput, Input, Switch, Textarea, Card, CardBody, IconButton } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { open } from '@tauri-apps/plugin-dialog';
import { configManager } from '../services/ConfigManager';
import type { DebugModeConfig, LanguageDebugTemplate } from '../types';
import { 
  LANGUAGE_TEMPLATE_LABELS, 
  DEFAULT_DEBUG_MODE_CONFIG,
  ALL_LANGUAGES,
  DEFAULT_LANGUAGE_TEMPLATES,
} from '../types';
import { createLogger } from '@/shared/utils/logger';
import './DebugConfig.scss';

const log = createLogger('DebugConfig');

const DebugConfig: React.FC = () => {
  const { t } = useTranslation('settings/debug');
  const [config, setConfig] = useState<DebugModeConfig>(DEFAULT_DEBUG_MODE_CONFIG);
  const [hasChanges, setHasChanges] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [expandedTemplates, setExpandedTemplates] = useState<Set<string>>(new Set());

  
  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      setLoading(true);
      const debugConfig = await configManager.getConfig<DebugModeConfig>('ai.debug_mode_config');
      if (debugConfig) {
        setConfig(debugConfig);
      }
    } catch (error) {
      log.error('Failed to load config', error);
      showMessage('error', t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  };

  const saveConfig = async () => {
    try {
      setSaving(true);
      await configManager.setConfig('ai.debug_mode_config', config);
      setHasChanges(false);
      showMessage('success', t('messages.saveSuccess'));
    } catch (error) {
      log.error('Failed to save config', error);
      showMessage('error', t('messages.saveFailed'));
    } finally {
      setSaving(false);
    }
  };

  const resetConfig = async () => {
    try {
      
      await configManager.resetConfig('ai.debug_mode_config');
      await loadConfig();
      setHasChanges(false);
      showMessage('success', t('messages.resetSuccess'));
    } catch (error) {
      log.error('Failed to reset config', error);
      showMessage('error', t('messages.resetFailed'));
    }
  };

  const updateConfig = useCallback((updates: Partial<DebugModeConfig>) => {
    setConfig(prev => ({ ...prev, ...updates }));
    setHasChanges(true);
  }, []);

  const updateTemplate = useCallback((language: string, updates: Partial<LanguageDebugTemplate>) => {
    setConfig(prev => ({
      ...prev,
      language_templates: {
        ...prev.language_templates,
        [language]: {
          ...prev.language_templates[language],
          ...updates
        }
      }
    }));
    setHasChanges(true);
  }, []);

  
  const toggleTemplateEnabled = useCallback(async (language: string, currentEnabled: boolean) => {
    const newEnabled = !currentEnabled;
    
    
    const newConfig = {
      ...config,
      language_templates: {
        ...config.language_templates,
        [language]: {
          ...config.language_templates[language],
          enabled: newEnabled
        }
      }
    };
    setConfig(newConfig);
    
    
    try {
      await configManager.setConfig('ai.debug_mode_config', newConfig);
      const templateName = config.language_templates[language]?.display_name || language;
      showMessage('success', newEnabled ? t('messages.templateEnabled', { name: templateName }) : t('messages.templateDisabled', { name: templateName }));
    } catch (error) {
      log.error('Failed to save template toggle', { language, error });
      
      setConfig(config);
      showMessage('error', t('messages.saveFailed'));
    }
  }, [config, t]);

  const showMessage = (type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  };

  const toggleTemplate = (language: string) => {
    setExpandedTemplates(prev => {
      const next = new Set(prev);
      if (next.has(language)) {
        next.delete(language);
      } else {
        next.add(language);
      }
      return next;
    });
  };

  
  const handleSelectLogPath = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{
          name: t('fileDialog.logFile'),
          extensions: ['log', 'txt', 'ndjson']
        }]
      });

      if (selected) {
        updateConfig({ log_path: selected });
        showMessage('success', t('messages.logPathUpdated'));
      }
    } catch (error) {
      showMessage('error', `${t('messages.selectFileFailed')}: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  
  const getTemplateEntries = useCallback((): [string, LanguageDebugTemplate][] => {
    const entries: [string, LanguageDebugTemplate][] = [];
    
    for (const lang of ALL_LANGUAGES) {
      const userTemplate = config.language_templates?.[lang];
      const defaultTemplate = DEFAULT_LANGUAGE_TEMPLATES[lang];
      
      const template = userTemplate || defaultTemplate;
      if (template) {
        entries.push([lang, template]);
      }
    }
    
    return entries;
  }, [config.language_templates]);

  const templateEntries = getTemplateEntries();

  if (loading) {
    return (
      <ConfigPageLayout className="bitfun-debug-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
        <ConfigPageContent>
          <div style={{ padding: '40px', textAlign: 'center', color: 'var(--color-text-muted)' }}>
            {t('messages.loading')}
          </div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-debug-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="bitfun-debug-config__content">
        
        {message && (
          <div className="bitfun-debug-config__message-container">
            <Alert
              type={message.type === 'success' ? 'success' : message.type === 'error' ? 'error' : 'info'}
              message={message.text}
            />
          </div>
        )}

        
        <div className="bitfun-debug-config__section">
          <div className="bitfun-debug-config__section-header">
            <div className="bitfun-debug-config__section-title">
              <h3>{t('sections.settings')}</h3>
            </div>
          </div>

          <div className="bitfun-debug-config__section-content">
              
              <div className="bitfun-debug-config__setting-item">
                <div className="bitfun-debug-config__setting-info">
                  <label className="bitfun-debug-config__setting-label">
                    {t('settings.logPath.label')}
                  </label>
                  <p className="bitfun-debug-config__setting-description">
                    {t('settings.logPath.description')}
                  </p>
                </div>
                <div className="bitfun-debug-config__input-group">
                  <Input
                    value={config.log_path}
                    onChange={(e) => updateConfig({ log_path: e.target.value })}
                    placeholder={t('settings.logPath.placeholder')}
                    variant="outlined"
                    inputSize="small"
                  />
                  <IconButton
                    variant="default"
                    size="small"
                    onClick={handleSelectLogPath}
                    tooltip={t('settings.logPath.browse')}
                  >
                    <FolderOpen size={16} />
                  </IconButton>
                </div>
              </div>

              
              <div className="bitfun-debug-config__setting-item">
                <div className="bitfun-debug-config__setting-info">
                  <label className="bitfun-debug-config__setting-label">
                    {t('settings.ingestPort.label')}
                  </label>
                  <p className="bitfun-debug-config__setting-description">
                    {t('settings.ingestPort.description')}
                  </p>
                </div>
                <NumberInput
                  value={config.ingest_port}
                  onChange={(v) => updateConfig({ ingest_port: v })}
                  min={1024}
                  max={65535}
                  step={1}
                  size="small"
                />
              </div>

              
              {hasChanges && (
                <div className="bitfun-debug-config__settings-actions">
                  <Button
                    variant="primary"
                    size="small"
                    onClick={saveConfig}
                    disabled={saving}
                  >
                    {saving ? t('actions.saving') : t('actions.save')}
                  </Button>
                  <Button
                    variant="secondary"
                    size="small"
                    onClick={loadConfig}
                    disabled={saving}
                  >
                    {t('actions.cancel')}
                  </Button>
                </div>
              )}
            </div>
        </div>

        
        <div className="bitfun-debug-config__section">
          <div className="bitfun-debug-config__section-header">
            <div className="bitfun-debug-config__section-title">
              <h3>{t('sections.templates')}</h3>
            </div>
          </div>

          <div className="bitfun-debug-config__section-content">
              <p className="bitfun-debug-config__setting-description" style={{ marginBottom: '16px' }}>
                {t('templates.description')}
              </p>

              <div className="bitfun-debug-config__templates-list">
                {templateEntries.map(([language, template]) => {
                  const isExpanded = expandedTemplates.has(language);
                  
                  return (
                    <Card
                      key={language}
                      variant="default"
                      padding="none"
                      interactive
                      className={`bitfun-debug-config__template-card ${isExpanded ? 'is-expanded' : ''}`}
                    >
                      <div 
                        className="bitfun-debug-config__template-header"
                        onClick={() => toggleTemplate(language)}
                      >
                        <div className="bitfun-debug-config__template-info">
                          <div onClick={(e) => e.stopPropagation()}>
                            <Switch
                              checked={template.enabled}
                              onChange={() => toggleTemplateEnabled(language, template.enabled)}
                              size="small"
                            />
                          </div>
                          <span className="bitfun-debug-config__template-name">
                            {template.display_name || LANGUAGE_TEMPLATE_LABELS[language] || language}
                          </span>
                        </div>
                        <ChevronDown 
                          size={16} 
                          className={`bitfun-debug-config__template-arrow ${isExpanded ? 'is-expanded' : ''}`}
                        />
                      </div>

                      {isExpanded && (
                        <CardBody className="bitfun-debug-config__template-content">
                          
                          <div className="bitfun-debug-config__template-field">
                            <Textarea
                              label={t('templates.instrumentation.label')}
                              value={template.instrumentation_template}
                              onChange={(e) => updateTemplate(language, { instrumentation_template: e.target.value })}
                              placeholder={t('templates.instrumentation.placeholder')}
                              hint={`${t('templates.instrumentation.placeholders')}: {LOCATION}, {MESSAGE}, {DATA}, {PORT}, {SESSION_ID}, {HYPOTHESIS_ID}, {RUN_ID}, {LOG_PATH}`}
                              variant="outlined"
                              autoResize
                            />
                          </div>

                          
                          <div className="bitfun-debug-config__template-field">
                            <label className="bitfun-debug-config__template-label">
                              {t('templates.region.label')}
                            </label>
                            <div className="bitfun-debug-config__region-inputs">
                              <Input
                                value={template.region_start}
                                onChange={(e) => updateTemplate(language, { region_start: e.target.value })}
                                placeholder={t('templates.region.startPlaceholder')}
                                variant="outlined"
                                inputSize="small"
                              />
                              <Input
                                value={template.region_end}
                                onChange={(e) => updateTemplate(language, { region_end: e.target.value })}
                                placeholder={t('templates.region.endPlaceholder')}
                                variant="outlined"
                                inputSize="small"
                              />
                            </div>
                          </div>

                          
                          {template.notes && template.notes.length > 0 && (
                            <div className="bitfun-debug-config__template-field">
                              <label className="bitfun-debug-config__template-label">
                                {t('templates.notes')}
                              </label>
                              <div className="bitfun-debug-config__template-notes">
                                {template.notes.map((note, idx) => (
                                  <span key={idx} className="bitfun-debug-config__template-note">
                                    {note}
                                  </span>
                                ))}
                              </div>
                            </div>
                          )}
                        </CardBody>
                      )}
                    </Card>
                  );
                })}
              </div>

              
              <div style={{ marginTop: '16px' }}>
                <Button
                  variant="secondary"
                  size="small"
                  onClick={resetConfig}
                >
                  <RefreshCw size={14} />
                  {t('templates.reset')}
                </Button>
              </div>
            </div>
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default DebugConfig;

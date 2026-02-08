/**
 * Add knowledge dialog component.
 * Supports Skill and RAG knowledge imports.
 */

import React, { useState, useCallback, useMemo } from 'react';
import {
  BookOpen,
  Brain,
  Database,
  FileText,
  Lightbulb,
  Library,
  Compass,
  Code,
  Terminal,
  Cpu,
  Globe,
  Search,
  Sparkles,
  Zap,
  Upload,
  Link,
  ChevronRight,
  AlertCircle,
  Check
} from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { useTranslation } from 'react-i18next';

const log = createLogger('AddKnowledgeDialog');
import { IconButton, Modal, Input, Textarea, Button, NumberInput } from '@/component-library';
import type { 
  KnowledgeBaseType, 
  AddKnowledgeFormData,
  SkillKnowledgeConfig,
  RAGKnowledgeConfig
} from '../../types/knowledge';
import './AddKnowledgeDialog.scss';

// Icon map for the selection grid.
const ICON_MAP: Record<string, React.ReactNode> = {
  BookOpen: <BookOpen size={16} />,
  Brain: <Brain size={16} />,
  Database: <Database size={16} />,
  FileText: <FileText size={16} />,
  Lightbulb: <Lightbulb size={16} />,
  Library: <Library size={16} />,
  Compass: <Compass size={16} />,
  Code: <Code size={16} />,
  Terminal: <Terminal size={16} />,
  Cpu: <Cpu size={16} />,
  Globe: <Globe size={16} />,
  Search: <Search size={16} />,
  Sparkles: <Sparkles size={16} />,
  Zap: <Zap size={16} />
};

export interface AddKnowledgeDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (data: AddKnowledgeFormData) => void;
}

type Step = 'type' | 'config' | 'confirm';

/**
 * Add knowledge dialog.
 */
export const AddKnowledgeDialog: React.FC<AddKnowledgeDialogProps> = ({
  isOpen,
  onClose,
  onConfirm
}) => {
  const { t } = useTranslation('panels/project-context');

  const [step, setStep] = useState<Step>('type');

  const [formData, setFormData] = useState<AddKnowledgeFormData>({
    name: '',
    description: '',
    type: 'skill',
    icon: 'BookOpen',
    tags: [],
    skillConfig: {
      format: 'markdown',
      autoSync: true
    },
    ragConfig: {
      endpoint: '',
      maxResults: 5,
      similarityThreshold: 0.7
    }
  });

  const [errors, setErrors] = useState<Record<string, string>>({});

  const resetState = useCallback(() => {
    setStep('type');
    setFormData({
      name: '',
      description: '',
      type: 'skill',
      icon: 'BookOpen',
      tags: [],
      skillConfig: {
        format: 'markdown',
        autoSync: true
      },
      ragConfig: {
        endpoint: '',
        maxResults: 5,
        similarityThreshold: 0.7
      }
    });
    setErrors({});
  }, []);

  const handleClose = useCallback(() => {
    resetState();
    onClose();
  }, [resetState, onClose]);

  const handleSelectType = useCallback((type: KnowledgeBaseType) => {
    setFormData(prev => ({ ...prev, type }));
    setStep('config');
  }, []);

  const updateField = useCallback(<K extends keyof AddKnowledgeFormData>(
    key: K,
    value: AddKnowledgeFormData[K]
  ) => {
    setFormData(prev => ({ ...prev, [key]: value }));
    // Clear the field error when user updates the value.
    if (errors[key]) {
      setErrors(prev => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
    }
  }, [errors]);

  const updateSkillConfig = useCallback(<K extends keyof SkillKnowledgeConfig>(
    key: K,
    value: SkillKnowledgeConfig[K]
  ) => {
    setFormData(prev => ({
      ...prev,
      skillConfig: { ...prev.skillConfig, [key]: value }
    }));
  }, []);

  const updateRAGConfig = useCallback(<K extends keyof RAGKnowledgeConfig>(
    key: K,
    value: RAGKnowledgeConfig[K]
  ) => {
    setFormData(prev => ({
      ...prev,
      ragConfig: { ...prev.ragConfig, [key]: value }
    }));
  }, []);

  const validateForm = useCallback((): boolean => {
    const newErrors: Record<string, string> = {};

    if (!formData.name.trim()) {
      newErrors.name = t('knowledgeTab.addDialog.validation.nameRequired');
    }

    if (formData.type === 'skill') {
      if (!formData.skillConfig?.filePath) {
        newErrors.skillConfig = t('knowledgeTab.addDialog.validation.fileRequired');
      }
    } else if (formData.type === 'rag') {
      if (!formData.ragConfig?.endpoint?.trim()) {
        newErrors.ragConfig = t('knowledgeTab.addDialog.validation.endpointRequired');
      }
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  }, [formData, t]);

  const handleNext = useCallback(() => {
    if (step === 'config') {
      if (validateForm()) {
        setStep('confirm');
      }
    }
  }, [step, validateForm]);

  const handleBack = useCallback(() => {
    if (step === 'config') {
      setStep('type');
    } else if (step === 'confirm') {
      setStep('config');
    }
  }, [step]);

  const handleSubmit = useCallback(() => {
    if (validateForm()) {
      onConfirm(formData);
      handleClose();
    }
  }, [validateForm, formData, onConfirm, handleClose]);

  const handleFileSelect = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [
          { name: 'Knowledge Files', extensions: ['md', 'txt', 'json', 'yaml', 'yml'] }
        ]
      });
      if (selected && typeof selected === 'string') {
        updateSkillConfig('filePath', selected);
        // Pick the format based on the file extension.
        const ext = selected.split('.').pop()?.toLowerCase();
        if (ext === 'json') {
          updateSkillConfig('format', 'json');
        } else if (ext === 'yaml' || ext === 'yml') {
          updateSkillConfig('format', 'yaml');
        } else if (ext === 'md') {
          updateSkillConfig('format', 'markdown');
        } else {
          updateSkillConfig('format', 'text');
        }
      }
    } catch (err) {
      log.error('Failed to select file', err);
    }
  }, [updateSkillConfig]);

  const stepTitle = useMemo(() => {
    switch (step) {
      case 'type':
        return t('knowledgeTab.addDialog.steps.type');
      case 'config':
        return formData.type === 'skill' 
          ? t('knowledgeTab.addDialog.steps.configSkill')
          : t('knowledgeTab.addDialog.steps.configRAG');
      case 'confirm':
        return t('knowledgeTab.addDialog.steps.confirm');
      default:
        return '';
    }
  }, [step, formData.type, t]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleClose}
      title={stepTitle}
      size="medium"
    >
      <div className="bitfun-add-knowledge-dialog">
        <div className="bitfun-add-knowledge-dialog__steps">
          <div 
            className={`bitfun-add-knowledge-dialog__step ${step === 'type' ? 'bitfun-add-knowledge-dialog__step--active' : ''} ${step !== 'type' ? 'bitfun-add-knowledge-dialog__step--done bitfun-add-knowledge-dialog__step--clickable' : ''}`}
            onClick={() => step !== 'type' && setStep('type')}
          >
            <span className="bitfun-add-knowledge-dialog__step-num">1</span>
            <span className="bitfun-add-knowledge-dialog__step-text">{t('knowledgeTab.addDialog.stepLabels.type')}</span>
          </div>
          <ChevronRight size={12} className="bitfun-add-knowledge-dialog__step-arrow" />
          <div 
            className={`bitfun-add-knowledge-dialog__step ${step === 'config' ? 'bitfun-add-knowledge-dialog__step--active' : ''} ${step === 'confirm' ? 'bitfun-add-knowledge-dialog__step--done bitfun-add-knowledge-dialog__step--clickable' : ''}`}
            onClick={() => step === 'confirm' && setStep('config')}
          >
            <span className="bitfun-add-knowledge-dialog__step-num">2</span>
            <span className="bitfun-add-knowledge-dialog__step-text">{t('knowledgeTab.addDialog.stepLabels.config')}</span>
          </div>
          <ChevronRight size={12} className="bitfun-add-knowledge-dialog__step-arrow" />
          <div className={`bitfun-add-knowledge-dialog__step ${step === 'confirm' ? 'bitfun-add-knowledge-dialog__step--active' : ''}`}>
            <span className="bitfun-add-knowledge-dialog__step-num">3</span>
            <span className="bitfun-add-knowledge-dialog__step-text">{t('knowledgeTab.addDialog.stepLabels.confirm')}</span>
          </div>
        </div>

        <div className="bitfun-add-knowledge-dialog__content">
          {step === 'type' && (
            <div className="bitfun-add-knowledge-dialog__type-selection">
              <div 
                className="bitfun-add-knowledge-dialog__type-card"
                onClick={() => handleSelectType('skill')}
              >
                <div className="bitfun-add-knowledge-dialog__type-icon">
                  <Brain size={24} />
                </div>
                <div className="bitfun-add-knowledge-dialog__type-info">
                  <h4>{t('knowledgeTab.addDialog.typeSelection.skill.title')}</h4>
                  <p>{t('knowledgeTab.addDialog.typeSelection.skill.description')}</p>
                </div>
                <ChevronRight size={16} className="bitfun-add-knowledge-dialog__type-arrow" />
              </div>

              <div 
                className="bitfun-add-knowledge-dialog__type-card"
                onClick={() => handleSelectType('rag')}
              >
                <div className="bitfun-add-knowledge-dialog__type-icon bitfun-add-knowledge-dialog__type-icon--rag">
                  <Database size={24} />
                </div>
                <div className="bitfun-add-knowledge-dialog__type-info">
                  <h4>{t('knowledgeTab.addDialog.typeSelection.rag.title')}</h4>
                  <p>{t('knowledgeTab.addDialog.typeSelection.rag.description')}</p>
                </div>
                <ChevronRight size={16} className="bitfun-add-knowledge-dialog__type-arrow" />
              </div>
            </div>
          )}

          {step === 'config' && (
            <div className="bitfun-add-knowledge-dialog__config">
              <Input
                label={t('knowledgeTab.addDialog.basicInfo.nameLabel')}
                value={formData.name}
                onChange={e => updateField('name', e.target.value)}
                placeholder={t('knowledgeTab.addDialog.basicInfo.namePlaceholder')}
                error={!!errors.name}
                errorMessage={errors.name}
              />

              <Textarea
                label={t('knowledgeTab.addDialog.basicInfo.descriptionLabel')}
                value={formData.description}
                onChange={e => updateField('description', e.target.value)}
                placeholder={t('knowledgeTab.addDialog.basicInfo.descriptionPlaceholder')}
                rows={2}
              />

              <div className="bitfun-add-knowledge-dialog__field">
                <label>{t('knowledgeTab.addDialog.basicInfo.iconLabel')}</label>
                <div className="bitfun-add-knowledge-dialog__icon-grid">
                  {Object.entries(ICON_MAP).map(([name, icon]) => (
                    <IconButton
                      key={name}
                      variant={formData.icon === name ? 'primary' : 'ghost'}
                      size="small"
                      onClick={() => updateField('icon', name)}
                    >
                      {icon}
                    </IconButton>
                  ))}
                </div>
              </div>

              {formData.type === 'skill' && (
                <div className="bitfun-add-knowledge-dialog__field">
                  <label>{t('knowledgeTab.addDialog.skillConfig.filePathLabel')} <span className="bitfun-add-knowledge-dialog__required">*</span></label>
                  <div className="bitfun-add-knowledge-dialog__file-row">
                    <Button variant="secondary" size="small" onClick={handleFileSelect}>
                      <Upload size={14} />
                      <span>{t('knowledgeTab.addDialog.skillConfig.selectFile')}</span>
                    </Button>
                    {formData.skillConfig?.filePath && (
                      <div className="bitfun-add-knowledge-dialog__file-path">
                        <FileText size={12} />
                        <span>{formData.skillConfig.filePath}</span>
                      </div>
                    )}
                  </div>
                  {errors.skillConfig && (
                    <span className="bitfun-add-knowledge-dialog__error">
                      <AlertCircle size={12} /> {errors.skillConfig}
                    </span>
                  )}
                </div>
              )}

              {formData.type === 'rag' && (
                <>
                  <Input
                    label={t('knowledgeTab.addDialog.ragConfig.endpointLabel')}
                    prefix={<Link size={14} />}
                    value={formData.ragConfig?.endpoint || ''}
                    onChange={e => updateRAGConfig('endpoint', e.target.value)}
                    placeholder={t('knowledgeTab.addDialog.ragConfig.endpointPlaceholder')}
                    error={!!errors.ragConfig}
                    errorMessage={errors.ragConfig}
                  />

                  <Input
                    label={t('knowledgeTab.addDialog.ragConfig.apiKeyLabel')}
                    type="password"
                    value={formData.ragConfig?.apiKey || ''}
                    onChange={e => updateRAGConfig('apiKey', e.target.value)}
                    placeholder={t('knowledgeTab.addDialog.ragConfig.apiKeyPlaceholder')}
                  />

                  <div className="bitfun-add-knowledge-dialog__field-row">
                    <NumberInput
                      label={t('knowledgeTab.addDialog.ragConfig.maxResultsLabel')}
                      min={1}
                      max={20}
                      value={formData.ragConfig?.maxResults || 5}
                      onChange={val => updateRAGConfig('maxResults', val)}
                    />
                    <NumberInput
                      label={t('knowledgeTab.addDialog.ragConfig.similarityThresholdLabel')}
                      min={0}
                      max={1}
                      step={0.1}
                      value={formData.ragConfig?.similarityThreshold || 0.7}
                      onChange={val => updateRAGConfig('similarityThreshold', val)}
                    />
                  </div>

                  <Input
                    label="Response JSONPath"
                    value={formData.ragConfig?.responsePath || ''}
                    onChange={e => updateRAGConfig('responsePath', e.target.value)}
                    placeholder="$.data.results"
                  />
                </>
              )}
            </div>
          )}

          {step === 'confirm' && (
            <div className="bitfun-add-knowledge-dialog__confirm">
              <div className="bitfun-add-knowledge-dialog__confirm-card">
                <div className="bitfun-add-knowledge-dialog__confirm-icon">
                  {ICON_MAP[formData.icon]}
                </div>
                <div className="bitfun-add-knowledge-dialog__confirm-info">
                  <h4>{formData.name}</h4>
                  <p>{formData.description || t('knowledgeTab.addDialog.confirm.description')}</p>
                  <div className="bitfun-add-knowledge-dialog__confirm-type">
                    <span className={`bitfun-add-knowledge-dialog__type-badge bitfun-add-knowledge-dialog__type-badge--${formData.type}`}>
                      {formData.type === 'skill' ? t('knowledgeTab.addDialog.confirm.typeSkill') : t('knowledgeTab.addDialog.confirm.typeRAG')}
                    </span>
                  </div>
                </div>
              </div>

              <div className="bitfun-add-knowledge-dialog__confirm-details">
                <h5>{t('knowledgeTab.addDialog.confirm.summaryTitle')}</h5>
                {formData.type === 'skill' && (
                  <ul>
                    {formData.skillConfig?.filePath && (
                      <li><FileText size={12} /> {t('knowledgeTab.addDialog.confirm.skillSourceFile')}: {formData.skillConfig.filePath}</li>
                    )}
                    <li><Check size={12} /> {t('knowledgeTab.addDialog.confirm.skillFormat')}: {formData.skillConfig?.format}</li>
                  </ul>
                )}
                {formData.type === 'rag' && (
                  <ul>
                    <li><Link size={12} /> {t('knowledgeTab.addDialog.confirm.ragEndpoint')}: {formData.ragConfig?.endpoint}</li>
                    <li><Check size={12} /> {t('knowledgeTab.addDialog.confirm.ragMaxResults')}: {formData.ragConfig?.maxResults}</li>
                    <li><Check size={12} /> {t('knowledgeTab.addDialog.confirm.ragThreshold')}: {formData.ragConfig?.similarityThreshold}</li>
                  </ul>
                )}
              </div>
            </div>
          )}
        </div>

        <div className="bitfun-add-knowledge-dialog__footer">
          {step !== 'type' && (
            <Button variant="secondary" onClick={handleBack}>
              {t('knowledgeTab.addDialog.buttons.back')}
            </Button>
          )}
          <div className="bitfun-add-knowledge-dialog__footer-spacer" />
          <Button variant="ghost" onClick={handleClose}>
            {t('knowledgeTab.addDialog.buttons.cancel')}
          </Button>
          {step === 'config' && (
            <Button variant="primary" onClick={handleNext}>
              {t('knowledgeTab.addDialog.buttons.next')}
            </Button>
          )}
          {step === 'confirm' && (
            <Button variant="primary" onClick={handleSubmit}>
              {t('knowledgeTab.addDialog.buttons.confirm')}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
};

export default AddKnowledgeDialog;

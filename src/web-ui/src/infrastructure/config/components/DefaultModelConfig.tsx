 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Image,
  Search,
  Mic,
  Palette,
  Layers,
  Phone,
} from 'lucide-react';
import { Select, CubeLoading } from '@/component-library';
import { notificationService } from '@/shared/notification-system';
import { configManager } from '../services/ConfigManager';
import type {
  AIModelConfig,
  DefaultModels,
  OptionalCapabilityModels,
  OptionalCapabilityType,
} from '../types';
import { createLogger } from '@/shared/utils/logger';
import './DefaultModelConfig.scss';

const log = createLogger('DefaultModelConfig');


const CAPABILITY_ICONS: Record<OptionalCapabilityType, React.ReactNode> = {
  image_understanding: <Image size={16} />,
  image_generation: <Palette size={16} />,
  search: <Search size={16} />,
  speech_recognition: <Mic size={16} />,
};


const OPTIONAL_CAPABILITY_TYPES: OptionalCapabilityType[] = [
  'image_understanding',
  'image_generation',
  'search',
  'speech_recognition'
];

export const DefaultModelConfig: React.FC = () => {
  const { t } = useTranslation('settings/default-model');
  
  
  const [loading, setLoading] = useState(true);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModels>({ primary: null, fast: null });
  const [optionalCapabilities, setOptionalCapabilities] = useState<OptionalCapabilityModels>({});
  
  
  const [optionalCollapsed, setOptionalCollapsed] = useState(true);

  
  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);

      
      const [allModels, defaultModelsConfig] = await Promise.all([
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<any>('ai.default_models') || {},
      ]);

      setModels(allModels);

      
      setDefaultModels({
        primary: defaultModelsConfig?.primary || null,
        fast: defaultModelsConfig?.fast || null,
      });

      
      setOptionalCapabilities({
        image_understanding: defaultModelsConfig?.image_understanding,
        image_generation: defaultModelsConfig?.image_generation,
        search: defaultModelsConfig?.search,
        speech_recognition: defaultModelsConfig?.speech_recognition,
      });
    } catch (error) {
      log.error('Failed to load data', error);
      notificationService.error(t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  };

  
  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    return models.find(m => m.id === modelId)?.name;
  }, [models]);

  
  const handleDefaultModelChange = async (slot: 'primary' | 'fast', modelId: string | number) => {
    try {
      const modelIdStr = modelId ? String(modelId) : null;

      
      const currentConfig = await configManager.getConfig<any>('ai.default_models') || {};

      
      await configManager.setConfig('ai.default_models', {
        ...currentConfig,
        [slot]: modelIdStr,
      });

      setDefaultModels(prev => ({
        ...prev,
        [slot]: modelIdStr,
      }));

      const modelName = getModelName(modelIdStr);
      notificationService.success(
        t('messages.modelUpdated', {
          slot: slot === 'primary' ? t('core.primary.label') : t('core.fast.label'),
          name: modelName || modelIdStr,
        }),
        { duration: 2000 }
      );
    } catch (error) {
      log.error('Failed to update default model', { slot, modelId: modelIdStr, error });
      notificationService.error(t('messages.updateFailed'));
    }
  };

  
  const handleCapabilityChange = async (capability: OptionalCapabilityType, modelId: string | number) => {
    try {
      const modelIdStr = modelId ? String(modelId) : null;

      
      const currentConfig = await configManager.getConfig<any>('ai.default_models') || {};

      
      await configManager.setConfig('ai.default_models', {
        ...currentConfig,
        [capability]: modelIdStr || undefined,
      });

      setOptionalCapabilities(prev => ({
        ...prev,
        [capability]: modelIdStr || undefined,
      }));

      notificationService.success(t('messages.capabilityUpdated'), { duration: 2000 });
    } catch (error) {
      log.error('Failed to update capability model', { capability, modelId: modelIdStr, error });
      notificationService.error(t('messages.updateFailed'));
    }
  };

  
  const enabledModels = models.filter(m => m.enabled);
  
  
  const getModelsForCapability = (capability: OptionalCapabilityType): AIModelConfig[] => {
    return enabledModels.filter(m => {
      switch (capability) {
        case 'image_understanding':
          return m.capabilities?.includes('image_understanding') || m.category === 'multimodal';
        case 'image_generation':
          return m.capabilities?.includes('image_generation') || m.category === 'image_generation';
        case 'search':
          return m.capabilities?.includes('search') || m.category === 'search_enhanced';
        case 'speech_recognition':
          return m.capabilities?.includes('speech_recognition') || m.category === 'speech_recognition';
        default:
          return true;
      }
    });
  };

  if (loading) {
    return (
      <div className="default-model-config__loading">
        <CubeLoading size="small" />
        <p>{t('loading')}</p>
      </div>
    );
  }

  if (models.length === 0) {
    return (
      <div className="default-model-config__empty">
        <Layers size={48} />
        <p>{t('empty.noModels')}</p>
      </div>
    );
  }

  return (
    <div className="default-model-config">
      
      <section className="default-model-config__section">
        <div className="default-model-config__core-section">
          
          <div className="default-model-config__model-slot">
            <div className="default-model-config__slot-header">
              <span className="default-model-config__slot-label">
                {t('core.primary.label')}
              </span>
              <span className="default-model-config__slot-badge">
                {t('core.required')}
              </span>
            </div>
            <p className="default-model-config__slot-description">
              {t('core.primary.description')}
            </p>
            <div className="default-model-config__slot-select">
              <Select
                value={defaultModels.primary || ''}
                onChange={(value) => handleDefaultModelChange('primary', value)}
                placeholder={t('core.primary.placeholder')}
                options={enabledModels.map(model => ({
                  label: model.name,
                  value: model.id!,
                }))}
              />
            </div>
          </div>
          
          
          <div className="default-model-config__model-slot">
            <div className="default-model-config__slot-header">
              <span className="default-model-config__slot-label">
                {t('core.fast.label')}
              </span>
              <span className="default-model-config__slot-badge default-model-config__slot-badge--optional">
                {t('core.optional')}
              </span>
            </div>
            <p className="default-model-config__slot-description">
              {t('core.fast.description')}
            </p>
            <div className="default-model-config__slot-select">
              <Select
                value={defaultModels.fast || ''}
                onChange={(value) => handleDefaultModelChange('fast', value)}
                placeholder={t('core.fast.placeholder')}
                options={[
                  { label: t('core.fast.notSet'), value: '' },
                  ...enabledModels.map(model => ({
                    label: model.name,
                    value: model.id!,
                  })),
                ]}
              />
            </div>
            <p className="default-model-config__slot-hint">
              {t('core.fast.hint')}
            </p>
          </div>
        </div>
      </section>

      
      <div className={`default-model-config__more-section ${!optionalCollapsed ? 'default-model-config__more-section--expanded' : ''}`}>
        <div 
          className="default-model-config__expand-trigger"
          onClick={() => setOptionalCollapsed(!optionalCollapsed)}
          role="button"
          tabIndex={0}
        >
          <span className="default-model-config__expand-line" />
          <span className="default-model-config__expand-text">
            {t('optional.title')}
          </span>
          <span className="default-model-config__expand-line" />
        </div>
        
        {!optionalCollapsed && (
          <div className="default-model-config__expand-content">
            <div className="default-model-config__capability-grid">
              {OPTIONAL_CAPABILITY_TYPES.map(capability => {
                const availableModels = getModelsForCapability(capability);
                const configuredModelId = optionalCapabilities[capability];
                const isConfigured = !!configuredModelId;
                
                return (
                  <div 
                    key={capability}
                    className={`default-model-config__capability-item ${isConfigured ? 'default-model-config__capability-item--configured' : ''}`}
                  >
                    <div className="default-model-config__capability-header">
                      <span className="default-model-config__capability-label">
                        {CAPABILITY_ICONS[capability]}
                        {' '}
                        {t(`optional.capabilities.${capability}.label`)}
                      </span>
                      <span className="default-model-config__capability-status">
                        {isConfigured 
                          ? getModelName(configuredModelId) 
                          : t('optional.notConfigured')
                        }
                      </span>
                    </div>
                    <p className="default-model-config__capability-description">
                      {t(`optional.capabilities.${capability}.description`)}
                    </p>
                    <Select
                      value={configuredModelId || ''}
                      onChange={(value) => handleCapabilityChange(capability, value)}
                      placeholder={t('optional.selectModel')}
                      disabled={availableModels.length === 0}
                      options={[
                        { label: t('optional.notSet'), value: '' },
                        ...availableModels.map(model => ({
                          label: model.name,
                          value: model.id!,
                        })),
                      ]}
                      size="small"
                    />
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>

    </div>
  );
};

export default DefaultModelConfig;

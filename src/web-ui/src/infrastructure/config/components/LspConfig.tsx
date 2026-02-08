 

import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Save, X } from 'lucide-react';
import { Alert, Switch, IconButton, Card } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { LspPluginList } from '@/tools/lsp';
import { lspService } from '@/tools/lsp/services/LspService';
import { open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import './LspConfig.scss';

const log = createLogger('LspConfig');

interface LspConfigProps {}

interface LspSettings {
  autoStartEnabled: boolean;
}

const DEFAULT_LSP_SETTINGS: LspSettings = {
  autoStartEnabled: true
};

const LSP_SETTINGS_KEY = 'bitfun_lsp_settings';

const LspConfig: React.FC<LspConfigProps> = () => {
  const { t } = useTranslation('settings/lsp');
  const [isInstalling, setIsInstalling] = useState(false);
  const [installMessage, setInstallMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [isInitializing, setIsInitializing] = useState(false);
  const [settings, setSettings] = useState<LspSettings>(DEFAULT_LSP_SETTINGS);
  const [hasSettingsChanges, setHasSettingsChanges] = useState(false);

  
  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = () => {
    try {
      const saved = localStorage.getItem(LSP_SETTINGS_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        setSettings({ ...DEFAULT_LSP_SETTINGS, ...parsed });
      }
    } catch (error) {
      log.error('Failed to load settings', error);
    }
  };

  const saveSettings = () => {
    try {
      localStorage.setItem(LSP_SETTINGS_KEY, JSON.stringify(settings));
      setHasSettingsChanges(false);
      setInstallMessage({ type: 'success', text: t('messages.settingsSaved') });
      setTimeout(() => setInstallMessage(null), 2000);
    } catch (error) {
      log.error('Failed to save settings', error);
      setInstallMessage({ type: 'error', text: t('messages.saveSettingsFailed') });
    }
  };

  const handleSettingChange = (key: keyof LspSettings, value: any) => {
    setSettings(prev => ({ ...prev, [key]: value }));
    setHasSettingsChanges(true);
  };

  
  const handleInitialize = async () => {
    setIsInitializing(true);
    setInstallMessage(null);
    
    try {
      await lspService.initialize();
      setInstallMessage({ type: 'success', text: t('messages.initSuccess') });
    } catch (error) {
      setInstallMessage({
        type: 'error',
        text: `${t('messages.initFailed')}: ${error instanceof Error ? error.message : String(error)}`
      });
    } finally {
      setIsInitializing(false);
    }
  };

  
  const handleInstallPlugin = async () => {
    try {
      
      const selected = await open({
        multiple: false,
        filters: [{
          name: t('fileDialog.pluginPackage'),
          extensions: ['vcpkg']
        }]
      });

      if (!selected) {
        return; 
      }

      setIsInstalling(true);
      setInstallMessage(null);

      
      await lspService.initialize();

      
      const pluginId = await lspService.installPlugin(selected as string);

      setInstallMessage({
        type: 'success',
        text: t('messages.installSuccess', { pluginId })
      });

      
      setTimeout(() => setInstallMessage(null), 3000);

    } catch (error) {
      setInstallMessage({
        type: 'error',
        text: `${t('messages.installFailed')}: ${error instanceof Error ? error.message : String(error)}`
      });
    } finally {
      setIsInstalling(false);
    }
  };

  return (
    <ConfigPageLayout className="bitfun-lsp-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="bitfun-lsp-config__content">
        <div className="bitfun-lsp-config__body">
          
          {installMessage && (
            <div className="bitfun-lsp-config__message-container">
              <Alert
                type={installMessage.type === 'success' ? 'success' : 'error'}
                message={installMessage.text}
              />
            </div>
          )}

          
          <div className="bitfun-lsp-config__settings">
            <div className="bitfun-lsp-config__settings-content">
              <Card variant="default" className="bitfun-lsp-config__setting-item">
                <Switch
                  checked={settings.autoStartEnabled}
                  onChange={(e) => handleSettingChange('autoStartEnabled', e.target.checked)}
                  label={t('settings.autoStart')}
                  description={t('settings.autoStartDesc')}
                  size="medium"
                />
              </Card>

              {hasSettingsChanges && (
                <div className="bitfun-lsp-config__settings-actions">
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={saveSettings}
                    tooltip={t('settings.saveTooltip')}
                  >
                    <Save size={18} />
                  </IconButton>
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={loadSettings}
                    tooltip={t('settings.cancelTooltip')}
                  >
                    <X size={18} />
                  </IconButton>
                </div>
              )}
            </div>
          </div>

          
          <div className="bitfun-lsp-config__plugins">
            <LspPluginList 
              onInitialize={handleInitialize}
              onInstallPlugin={handleInstallPlugin}
              isInitializing={isInitializing}
              isInstalling={isInstalling}
            />
          </div>
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default LspConfig;


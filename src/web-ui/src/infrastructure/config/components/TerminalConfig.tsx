

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Download } from 'lucide-react';
import {
  Alert,
  Select,
  ConfigPageLoading,
  ConfigPageMessage,
  ConfigPageRefreshButton,
} from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { configManager } from '../services/ConfigManager';
import { getTerminalService } from '@/tools/terminal';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';
import type { TerminalConfig as TerminalConfigType } from '../types';
import type { ShellInfo } from '@/tools/terminal/types/session';
import './TerminalConfig.scss';

const log = createLogger('TerminalConfig');

const TerminalConfig: React.FC = () => {
  const { t } = useTranslation('settings/terminal');
  const [defaultShell, setDefaultShell] = useState<string>('');
  const [availableShells, setAvailableShells] = useState<ShellInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [platform, setPlatform] = useState<string>('');

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);

      const [terminalConfig, shells, systemInfo] = await Promise.all([
        configManager.getConfig<TerminalConfigType>('terminal'),
        getTerminalService().getAvailableShells(),
        systemAPI.getSystemInfo().catch(() => ({ platform: '' }))
      ]);

      setDefaultShell(terminalConfig?.default_shell || '');

      const availableOnly = shells.filter(s => s.available);
      setAvailableShells(availableOnly);

      setPlatform(systemInfo.platform || '');
    } catch (error) {
      log.error('Failed to load terminal config data', error);
      showMessage('error', t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  };

  const handleShellChange = useCallback(async (value: string) => {
    try {
      setSaving(true);
      setDefaultShell(value);

      await configManager.setConfig('terminal.default_shell', value);

      configManager.clearCache();

      showMessage('success', t('messages.updated'));
    } catch (error) {
      log.error('Failed to save terminal config', { shell: value, error });
      showMessage('error', t('messages.saveFailed'));
    } finally {
      setSaving(false);
    }
  }, [t]);

  const showMessage = (type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  };

  const handleRefresh = useCallback(async () => {
    await loadData();
    showMessage('info', t('messages.refreshed'));
  }, [t]);

  const shouldShowPowerShellCoreRecommendation = useCallback(() => {
    const isWindows = platform === 'windows';
    if (!isWindows) return false;

    const hasPowerShellCore = availableShells.some(
      shell => shell.shellType === 'PowerShellCore'
    );

    return !hasPowerShellCore;
  }, [availableShells, platform]);

  const shellOptions = [
    { value: '', label: t('terminal.autoDetect') },
    ...availableShells.map(shell => ({
      value: shell.shellType,
      label: `${shell.name}${shell.version ? ` (${shell.version})` : ''}`
    }))
  ];

  if (loading) {
    return (
      <ConfigPageLayout className="bitfun-terminal-config">
        <ConfigPageHeader
          title={t('title')}
          subtitle={t('subtitle')}
        />
        <ConfigPageContent>
          <ConfigPageLoading text={t('messages.loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-terminal-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="bitfun-terminal-config__content">

        <ConfigPageMessage message={message} />

        {shouldShowPowerShellCoreRecommendation() && (
          <div className="bitfun-terminal-config__message-container">
            <Alert
              type="info"
              message={
                <span style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                  <Download size={16} />
                  <span>{t('recommendations.pwsh.prefix')} </span>
                  <strong>{t('recommendations.pwsh.name')}</strong>
                  <span>{t('recommendations.pwsh.suffix')}</span>
                  <a
                    href="https://aka.ms/PSWindows"
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{
                      color: 'var(--color-primary)',
                      textDecoration: 'underline',
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px'
                    }}
                  >
                    {t('recommendations.pwsh.link')}
                  </a>
                </span>
              }
            />
          </div>
        )}

        <ConfigPageSection
          title={t('sections.defaultTerminal')}
          extra={(
            <ConfigPageRefreshButton
              tooltip={t('terminal.refreshTooltip')}
              onClick={handleRefresh}
              loading={loading}
              disabled={loading}
            />
          )}
        >
          <ConfigPageRow
            label={t('sections.defaultTerminal')}
            description={t('terminal.description')}
            align="center"
          >
            <div className="bitfun-terminal-config__select-wrapper">
              {availableShells.length > 0 ? (
                <Select
                  value={defaultShell}
                  onChange={(v) => handleShellChange(v as string)}
                  options={shellOptions}
                  placeholder={t('terminal.placeholder')}
                  disabled={saving}
                />
              ) : (
                <div className="bitfun-terminal-config__no-shells">
                  {t('terminal.noShells')}
                </div>
              )}
            </div>
          </ConfigPageRow>

          {platform === 'windows' && defaultShell === 'Cmd' && (
            <div className="bitfun-terminal-config__inline-alert">
              <Alert type="warning" message={t('terminal.warnings.cmd')} />
            </div>
          )}
          {platform === 'windows' && defaultShell === 'Bash' && (
            <div className="bitfun-terminal-config__inline-alert">
              <Alert type="warning" message={t('terminal.warnings.gitBash')} />
            </div>
          )}
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default TerminalConfig;

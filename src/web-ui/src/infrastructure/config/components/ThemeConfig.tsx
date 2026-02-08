 

import { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Upload } from 'lucide-react';
import { useTheme, useThemeManagement, ThemeMetadata, ThemeConfig as ThemeConfigType } from '@/infrastructure/theme';
import { themeService } from '@/infrastructure/theme/core/ThemeService';
import { useLanguageSelector } from '@/infrastructure/i18n';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { IconButton, Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './ThemeConfig.scss';

const log = createLogger('ThemeConfig');

export function ThemeConfig() {
  const { t } = useTranslation('settings/theme');
  const { themeId, themes, setTheme, loading } = useTheme();
  const { removeTheme, exportTheme, importTheme } = useThemeManagement();
  const { currentLanguage, supportedLocales, selectLanguage, isChanging } = useLanguageSelector();
  const [importing, setImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  
  
  const handleThemeChange = async (newThemeId: string) => {
    await setTheme(newThemeId);
  };
  
  
  const handleDeleteTheme = async (themeIdToDelete: string) => {
    const themeName = themes.find(t => t.id === themeIdToDelete)?.name;
    if (window.confirm(t('theme.confirmDelete', { name: themeName }))) {
      await removeTheme(themeIdToDelete);
    }
  };
  
  
  const handleExportTheme = (themeIdToExport: string) => {
    const exportData = exportTheme(themeIdToExport);
    if (exportData) {
      const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${themeIdToExport}-theme.json`;
      a.click();
      URL.revokeObjectURL(url);
    }
  };
  
  
  const handleImportTheme = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    
    setImporting(true);
    try {
      const text = await file.text();
      const themeData = JSON.parse(text);
      await importTheme(themeData);
      alert(t('theme.importSuccess'));
    } catch (error) {
      log.error('Failed to import theme', error);
      alert(t('theme.importFailed'));
    } finally {
      setImporting(false);
      
      event.target.value = '';
    }
  };
  
  
  const handleImportClick = () => {
    fileInputRef.current?.click();
  };
  
  return (
    <ConfigPageLayout className="theme-config">
      <ConfigPageHeader
        title={t('theme.title')}
        subtitle={t('theme.subtitle')}
      />
      
      <ConfigPageContent className="theme-config__content">
      
      <div className="theme-config__section">
        <div className="theme-config__section-header">
          <h3 className="theme-config__section-title">{t('theme.language')}</h3>
        </div>
        <div className="theme-config__language-grid">
          {supportedLocales.map((locale) => (
            <button
              key={locale.id}
              className={`theme-config__language-btn ${locale.id === currentLanguage ? 'theme-config__language-btn--active' : ''}`}
              onClick={() => selectLanguage(locale.id)}
              disabled={isChanging}
            >
              <span className="theme-config__language-native">{locale.nativeName}</span>
              <span className="theme-config__language-english">{locale.englishName}</span>
              {locale.id === currentLanguage && (
                <span className="theme-config__language-check">✓</span>
              )}
            </button>
          ))}
        </div>
      </div>
      
      
      <div className="theme-config__section">
        <div className="theme-config__section-header">
          <h3 className="theme-config__section-title">{t('theme.themes')}</h3>
          <IconButton
            variant="ghost"
            size="small"
            onClick={handleImportClick}
            disabled={importing || loading}
            isLoading={importing}
            title={importing ? t('theme.importing') : t('theme.importTheme')}
          >
            <Upload size={16} />
          </IconButton>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json"
            onChange={handleImportTheme}
            style={{ display: 'none' }}
          />
        </div>
        <div className="theme-config__grid">
          {themes.map((t) => (
            <ThemeCard
              key={t.id}
              theme={t}
              active={t.id === themeId}
              onSelect={() => handleThemeChange(t.id)}
              onDelete={t.builtin ? undefined : () => handleDeleteTheme(t.id)}
              onExport={() => handleExportTheme(t.id)}
              disabled={loading}
            />
          ))}
        </div>
      </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
}

 
interface ThemePreviewThumbnailProps {
  theme: ThemeConfigType;
}

function ThemePreviewThumbnail({ theme }: ThemePreviewThumbnailProps) {
  const { colors } = theme;
  
  return (
    <div 
      className="theme-preview-thumbnail"
      style={{
        background: colors.background.primary,
        borderColor: colors.border.base,
      }}
    >
      
      <div 
        className="theme-preview-thumbnail__titlebar"
        style={{ 
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        
        <div className="theme-preview-thumbnail__menu">
          <span 
            className="theme-preview-thumbnail__menu-dot"
            style={{ background: colors.accent['500'] }}
          />
        </div>
        
        
        <div 
          className="theme-preview-thumbnail__title"
          style={{ color: colors.text.muted }}
        >
          BitFun
        </div>
        
        
        <div className="theme-preview-thumbnail__window-controls">
          
          <span 
            className="theme-preview-thumbnail__window-btn"
            style={{ color: colors.text.secondary }}
          >
            <svg width="8" height="8" viewBox="0 0 14 14" fill="none">
              <line x1="3" y1="7" x2="11" y2="7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
          </span>
          
          <span 
            className="theme-preview-thumbnail__window-btn"
            style={{ color: colors.text.secondary }}
          >
            <svg width="8" height="8" viewBox="0 0 12 12" fill="none">
              <rect x="2" y="2" width="8" height="8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </span>
          
          <span 
            className="theme-preview-thumbnail__window-btn theme-preview-thumbnail__window-btn--close"
            style={{ color: colors.text.secondary }}
          >
            <svg width="8" height="8" viewBox="0 0 14 14" fill="none">
              <line x1="3" y1="3" x2="11" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
              <line x1="11" y1="3" x2="3" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
          </span>
        </div>
      </div>
      
      
      <div className="theme-preview-thumbnail__main">
        
        <div 
          className="theme-preview-thumbnail__sidebar"
          style={{ 
            background: colors.background.secondary,
            borderColor: colors.border.subtle,
          }}
        >
          
          <div className="theme-preview-thumbnail__tree-item">
            <span 
              className="theme-preview-thumbnail__folder-icon"
              style={{ background: colors.accent['500'] }}
            />
            <span 
              className="theme-preview-thumbnail__tree-text"
              style={{ background: colors.text.secondary }}
            />
          </div>
          
          {[1, 2, 3].map((i) => (
            <div key={i} className="theme-preview-thumbnail__tree-item theme-preview-thumbnail__tree-item--file">
              <span 
                className="theme-preview-thumbnail__file-icon"
                style={{ background: colors.semantic.info }}
              />
              <span 
                className="theme-preview-thumbnail__tree-text theme-preview-thumbnail__tree-text--short"
                style={{ background: colors.text.muted }}
              />
            </div>
          ))}
        </div>
        
        
        <div 
          className="theme-preview-thumbnail__chat"
          style={{ background: colors.background.flowchat }}
        >
          
          <div 
            className="theme-preview-thumbnail__message theme-preview-thumbnail__message--user"
            style={{ 
              background: colors.accent['200'],
              borderColor: colors.accent['400'],
            }}
          >
            <div 
              className="theme-preview-thumbnail__message-line"
              style={{ background: colors.text.primary }}
            />
          </div>
          
          <div 
            className="theme-preview-thumbnail__message theme-preview-thumbnail__message--ai"
            style={{ 
              background: colors.element.subtle,
              borderColor: colors.border.subtle,
            }}
          >
            <div 
              className="theme-preview-thumbnail__message-line"
              style={{ background: colors.text.secondary }}
            />
            <div 
              className="theme-preview-thumbnail__message-line theme-preview-thumbnail__message-line--short"
              style={{ background: colors.text.muted }}
            />
          </div>
          
          <div 
            className="theme-preview-thumbnail__code-block"
            style={{ 
              background: colors.background.tertiary,
              borderColor: colors.border.base,
            }}
          >
            <div 
              className="theme-preview-thumbnail__code-line"
              style={{ background: colors.purple?.['500'] || colors.accent['500'] }}
            />
            <div 
              className="theme-preview-thumbnail__code-line theme-preview-thumbnail__code-line--long"
              style={{ background: colors.semantic.success }}
            />
          </div>
        </div>
        
        
        <div 
          className="theme-preview-thumbnail__editor"
          style={{ 
            background: colors.background.workbench,
            borderColor: colors.border.subtle,
          }}
        >
          
          <div 
            className="theme-preview-thumbnail__tabs"
            style={{ 
              background: colors.background.secondary,
              borderColor: colors.border.subtle,
            }}
          >
            <span 
              className="theme-preview-thumbnail__tab theme-preview-thumbnail__tab--active"
              style={{ 
                background: colors.background.primary,
                borderColor: colors.accent['500'],
              }}
            />
            <span 
              className="theme-preview-thumbnail__tab"
              style={{ background: colors.element.subtle }}
            />
          </div>
          
          <div className="theme-preview-thumbnail__code-content">
            {[1, 2, 3, 4, 5].map((i) => (
              <div key={i} className="theme-preview-thumbnail__editor-line">
                <span 
                  className="theme-preview-thumbnail__line-number"
                  style={{ background: colors.text.disabled }}
                />
                <span 
                  className="theme-preview-thumbnail__line-code"
                  style={{ 
                    background: i % 2 === 0 ? colors.accent['500'] : colors.text.secondary,
                    width: `${30 + (i * 8) % 40}%`,
                  }}
                />
              </div>
            ))}
          </div>
        </div>
      </div>
      
      
      <div 
        className="theme-preview-thumbnail__statusbar"
        style={{ 
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        
        <div className="theme-preview-thumbnail__status-section">
          <span 
            className="theme-preview-thumbnail__status-icon"
            style={{ background: colors.accent['500'] }}
          />
          <span 
            className="theme-preview-thumbnail__status-text"
            style={{ background: colors.text.muted }}
          />
        </div>
        
        
        <div className="theme-preview-thumbnail__status-section">
          <span 
            className="theme-preview-thumbnail__git-icon"
            style={{ color: colors.git.branch }}
          >
            <svg width="7" height="7" viewBox="0 0 16 16" fill="none">
              <circle cx="4" cy="4" r="2" stroke="currentColor" strokeWidth="1.5"/>
              <circle cx="12" cy="12" r="2" stroke="currentColor" strokeWidth="1.5"/>
              <circle cx="12" cy="4" r="2" stroke="currentColor" strokeWidth="1.5"/>
              <path d="M4 6v4c0 1.1.9 2 2 2h4" stroke="currentColor" strokeWidth="1.5"/>
            </svg>
          </span>
          <span 
            className="theme-preview-thumbnail__status-text theme-preview-thumbnail__status-text--branch"
            style={{ background: colors.git.branch }}
          />
        </div>
        
        
        <span 
          className="theme-preview-thumbnail__status-icon theme-preview-thumbnail__status-icon--notification"
          style={{ background: colors.semantic.info }}
        />
      </div>
    </div>
  );
}

 
interface ThemeCardProps {
  theme: ThemeMetadata;
  active: boolean;
  onSelect: () => void;
  onDelete?: () => void;
  onExport: () => void;
  disabled?: boolean;
}

function ThemeCard({ theme, active, onSelect, onDelete, onExport, disabled }: ThemeCardProps) {
  const { t } = useTranslation('settings/theme');
  const [showActions, setShowActions] = useState(false);
  
  
  const fullTheme = themeService.getTheme(theme.id);
  
  
  const primaryColor = fullTheme?.colors.accent['500'] || '#3b82f6';
  const secondaryColor = fullTheme?.colors.purple?.['500'] || fullTheme?.colors.accent['600'] || '#8b5cf6';
  const tertiaryColor = fullTheme?.colors.semantic.success || '#22c55e';
  
  
  const bgPrimary = fullTheme?.colors.background.primary || (theme.type === 'dark' ? '#0a0a0a' : '#ffffff');
  const bgSecondary = fullTheme?.colors.background.secondary || (theme.type === 'dark' ? '#1a1a1a' : '#f5f5f5');
  
  
  const i18nKey = `theme.presets.${theme.id}`;
  const themeName = theme.builtin 
    ? t(`${i18nKey}.name`, { defaultValue: theme.name }) 
    : theme.name;
  const themeDescription = theme.builtin 
    ? t(`${i18nKey}.description`, { defaultValue: theme.description || '' }) 
    : theme.description;
  
  
  const cardContent = (
    <div
      className={`theme-card ${active ? 'theme-card--active' : ''} ${disabled ? 'theme-card--disabled' : ''}`}
      onClick={!disabled ? onSelect : undefined}
      onMouseEnter={() => setShowActions(true)}
      onMouseLeave={() => setShowActions(false)}
    >
      
      <div 
        className="theme-card__preview"
        style={{ 
          background: `linear-gradient(135deg, ${bgPrimary} 0%, ${bgSecondary} 100%)` 
        }}
      >
        <div 
          className="theme-card__preview-dot" 
          style={{ background: primaryColor }}
        ></div>
        <div 
          className="theme-card__preview-dot" 
          style={{ background: secondaryColor }}
        ></div>
        <div 
          className="theme-card__preview-dot" 
          style={{ background: tertiaryColor }}
        ></div>
      </div>
      
      
      <div className="theme-card__info">
        <div className="theme-card__name">{themeName}</div>
        {themeDescription && (
          <div className="theme-card__description">{themeDescription}</div>
        )}
        <div className="theme-card__meta">
          {theme.builtin && (
            <span className="theme-card__badge">{t('theme.builtIn')}</span>
          )}
          {theme.author && (
            <span className="theme-card__author">{theme.author}</span>
          )}
        </div>
      </div>
      
      
      {active && (
        <div className="theme-card__active-indicator">
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
            <path
              d="M13.5 4L6 11.5L2.5 8"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>
      )}
      
      
      {showActions && !active && (
        <div className="theme-card__actions" onClick={(e) => e.stopPropagation()}>
          <Tooltip content={t('theme.export')}>
            <button
              className="theme-card__action-btn"
              onClick={onExport}
            >
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path
                  d="M8 12V4M8 12L11 9M8 12L5 9"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M2 14H14"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
              </svg>
            </button>
          </Tooltip>
          
          {onDelete && (
            <Tooltip content={t('theme.delete')}>
              <button
                className="theme-card__action-btn theme-card__action-btn--danger"
                onClick={onDelete}
              >
                  <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                    <path
                      d="M3 4H13M5 4V3C5 2.44772 5.44772 2 6 2H10C10.5523 2 11 2.44772 11 3V4M6.5 7V11M9.5 7V11M4 4H12V13C12 13.5523 11.5523 14 11 14H5C4.44772 14 4 13.5523 4 13V4Z"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </button>
              </Tooltip>
          )}
        </div>
      )}
    </div>
  );
  
  
  if (fullTheme) {
    return (
      <Tooltip 
        content={<ThemePreviewThumbnail theme={fullTheme} />}
        placement="right"
        delay={400}
        className="theme-preview-tooltip"
      >
        {cardContent}
      </Tooltip>
    );
  }
  
  return cardContent;
}



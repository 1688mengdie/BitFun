/**
 * Template picker panel.
 * Floating panel for quick prompt template selection.
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Star, X } from 'lucide-react';
import { Search } from '@/component-library';
import { promptTemplateService } from '@/infrastructure/services/PromptTemplateService';
import { PromptTemplate, TemplateSearchResult } from '@/shared/types/prompt-template';
import { parseTemplate } from '@/shared/utils/templateParser';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import './TemplatePickerPanel.scss';

export interface TemplatePickerPanelProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (template: PromptTemplate) => void;
}

export const TemplatePickerPanel: React.FC<TemplatePickerPanelProps> = ({
  isOpen,
  onClose,
  onSelect
}) => {
  const { t } = useI18n('settings/prompt-templates');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<TemplateSearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [recentTemplates, setRecentTemplates] = useState<PromptTemplate[]>([]);
  const [favoriteTemplates, setFavoriteTemplates] = useState<PromptTemplate[]>([]);
  
  const searchInputRef = useRef<any>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isOpen) {
      loadTemplates();
      setSearchQuery('');
      setSelectedIndex(0);
      
      // Focus the search input after mount.
      setTimeout(() => {
        searchInputRef.current?.focus?.();
      }, 100);
    }
  }, [isOpen]);

  const loadTemplates = useCallback(() => {
    const recent = promptTemplateService.getRecentTemplates(5);
    const favorite = promptTemplateService.getFavoriteTemplates();
    const all = promptTemplateService.getAllTemplates();

    setRecentTemplates(recent);
    setFavoriteTemplates(favorite);
    
    setSearchResults(
      all.map(template => ({
        template,
        matchScore: 1,
        matchedFields: []
      }))
    );
  }, []);

  useEffect(() => {
    if (!isOpen) return;

    const results = promptTemplateService.searchTemplates(searchQuery);
    setSearchResults(results);
    setSelectedIndex(0);
  }, [searchQuery, isOpen]);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSelectedIndex(prev => 
            Math.min(prev + 1, searchResults.length - 1)
          );
          break;
        
        case 'ArrowUp':
          e.preventDefault();
          setSelectedIndex(prev => Math.max(prev - 1, 0));
          break;
        
        case 'Enter':
          e.preventDefault();
          if (searchResults[selectedIndex]) {
            handleSelect(searchResults[selectedIndex].template);
          }
          break;
        
        case 'Escape':
          e.preventDefault();
          onClose();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, searchResults, selectedIndex, onClose]);

  useEffect(() => {
    if (!listRef.current) return;

    const selectedElement = listRef.current.querySelector(
      `[data-index="${selectedIndex}"]`
    ) as HTMLElement;

    if (selectedElement) {
      selectedElement.scrollIntoView({
        block: 'nearest',
        behavior: 'smooth'
      });
    }
  }, [selectedIndex]);

  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 100);

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen, onClose]);

  const handleSelect = useCallback((template: PromptTemplate) => {
    promptTemplateService.recordUsage(template.id);
    onSelect(template);
    onClose();
  }, [onSelect, onClose]);

  if (!isOpen) {
    return null;
  }

  return (
    <div className="template-picker-overlay">
      <div className="template-picker-panel" ref={panelRef}>
        <div className="template-picker-panel__header">
          <div className="template-picker-panel__search">
            <Search
              ref={searchInputRef}
              placeholder={t('picker.searchPlaceholder')}
              value={searchQuery}
              onChange={(val) => setSearchQuery(val)}
              clearable
              size="small"
              autoFocus
            />
          </div>
          <Tooltip content={t('picker.closeTooltip')}>
            <button
              className="template-picker-panel__close-button"
              onClick={onClose}
            >
              <X size={16} />
            </button>
          </Tooltip>
        </div>

        <div className="template-picker-panel__content" ref={listRef}>
          {searchQuery === '' && recentTemplates.length > 0 && (
            <div className="template-picker-panel__section">
              <div className="template-picker-panel__section-title">
                <span>{t('picker.sectionRecent')}</span>
              </div>
              {recentTemplates.map((template) => {
                const index = searchResults.findIndex(r => r.template.id === template.id);
                return (
                  <TemplateItem
                    key={template.id}
                    template={template}
                    isSelected={index === selectedIndex}
                    onClick={() => handleSelect(template)}
                    dataIndex={index}
                  />
                );
              })}
            </div>
          )}

          {searchQuery === '' && favoriteTemplates.length > 0 && (
            <div className="template-picker-panel__section">
              <div className="template-picker-panel__section-title">
                <span>{t('picker.sectionFavorites')}</span>
              </div>
              {favoriteTemplates.map((template) => {
                const index = searchResults.findIndex(r => r.template.id === template.id);
                return (
                  <TemplateItem
                    key={template.id}
                    template={template}
                    isSelected={index === selectedIndex}
                    onClick={() => handleSelect(template)}
                    dataIndex={index}
                  />
                );
              })}
            </div>
          )}

          {searchQuery !== '' && searchResults.length === 0 && (
            <div className="template-picker-panel__empty">
              <p>{t('picker.emptyNoMatch')}</p>
              <p className="template-picker-panel__empty-hint">
                {t('picker.emptyHint')}
              </p>
            </div>
          )}

          {searchQuery !== '' && searchResults.length > 0 && (
            <div className="template-picker-panel__section">
              <div className="template-picker-panel__section-title">
                <span>{t('picker.sectionSearchResults')}</span>
                <span className="template-picker-panel__count">
                  {searchResults.length}
                </span>
              </div>
              {searchResults.map((result, index) => (
                <TemplateItem
                  key={result.template.id}
                  template={result.template}
                  isSelected={index === selectedIndex}
                  onClick={() => handleSelect(result.template)}
                  matchedFields={result.matchedFields}
                  dataIndex={index}
                />
              ))}
            </div>
          )}

          {searchQuery === '' && searchResults.length > 0 && (
            <div className="template-picker-panel__section">
              <div className="template-picker-panel__section-title">
                <span>{t('picker.sectionAllTemplates')}</span>
                <span className="template-picker-panel__count">
                  {searchResults.length}
                </span>
              </div>
              {searchResults.map((result, index) => (
                <TemplateItem
                  key={result.template.id}
                  template={result.template}
                  isSelected={index === selectedIndex}
                  onClick={() => handleSelect(result.template)}
                  dataIndex={index}
                />
              ))}
            </div>
          )}
        </div>

        <div className="template-picker-panel__footer">
          <div className="template-picker-panel__hints">
            <span><kbd>↑↓</kbd> {t('picker.hintNavigate')}</span>
            <span><kbd>Enter</kbd> {t('picker.hintSelect')}</span>
            <span><kbd>Esc</kbd> {t('picker.hintClose')}</span>
          </div>
        </div>
      </div>
    </div>
  );
};

// Template item component.
interface TemplateItemProps {
  template: PromptTemplate;
  isSelected: boolean;
  onClick: () => void;
  matchedFields?: string[];
  dataIndex: number;
}

const TemplateItem: React.FC<TemplateItemProps> = ({
  template,
  isSelected,
  onClick,
  matchedFields,
  dataIndex
}) => {
  const { t } = useI18n('settings/prompt-templates');
  // Render a preview with highlighted placeholders.
  const renderTemplatePreview = (content: string) => {
    const placeholders = parseTemplate(content);
    
    if (placeholders.length === 0) {
      return content.length > 100 ? content.substring(0, 100) + '...' : content;
    }
    
    // Truncate to 100 chars and only highlight placeholders in range.
    const displayContent = content.length > 100 ? content.substring(0, 100) + '...' : content;
    const elements: React.ReactNode[] = [];
    let lastIndex = 0;
    
    placeholders.forEach((placeholder, index) => {
      if (placeholder.startIndex >= 100) return;
      
      if (placeholder.startIndex > lastIndex) {
        elements.push(
          <span key={`text-${index}`}>
            {displayContent.substring(lastIndex, placeholder.startIndex)}
          </span>
        );
      }
      
      const endIndex = Math.min(placeholder.endIndex, 100);
      elements.push(
        <span key={`placeholder-${index}`} className="template-picker-panel__item-placeholder">
          {displayContent.substring(placeholder.startIndex, endIndex)}
        </span>
      );
      
      lastIndex = endIndex;
    });
    
    if (lastIndex < displayContent.length) {
      elements.push(
        <span key="text-end">
          {displayContent.substring(lastIndex)}
        </span>
      );
    }
    
    return <>{elements}</>;
  };
  
  return (
    <div
      className={`template-picker-panel__item ${isSelected ? 'template-picker-panel__item--selected' : ''}`}
      onClick={onClick}
      data-index={dataIndex}
    >
      <div className="template-picker-panel__item-header">
        <div className="template-picker-panel__item-title">
          {template.isFavorite && (
            <Star size={12} className="template-picker-panel__item-star" fill="currentColor" />
          )}
          <span>{template.name}</span>
        </div>
        <div className="template-picker-panel__item-meta">
          {template.category && (
            <span className="template-picker-panel__item-category">
              {template.category}
            </span>
          )}
          {template.shortcut && (
            <kbd className="template-picker-panel__item-shortcut">
              {template.shortcut}
            </kbd>
          )}
        </div>
      </div>
      
      {template.description && (
        <div className="template-picker-panel__item-description">
          {template.description}
        </div>
      )}
      
      <div className="template-picker-panel__item-preview">
        {renderTemplatePreview(template.content)}
      </div>

      {matchedFields && matchedFields.length > 0 && (
        <div className="template-picker-panel__item-matches">
          {t('picker.matches', { fields: matchedFields.join(', ') })}
        </div>
      )}

      {template.usageCount > 0 && (
        <div className="template-picker-panel__item-usage">
          {t('picker.usageCount', { count: template.usageCount })}
        </div>
      )}
    </div>
  );
};

export default TemplatePickerPanel;


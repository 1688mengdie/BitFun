/**
 * DocumentItem - document row component.
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  FileCheck,
  FilePlus2,
  Sparkles,
  Plus,
  X,
  Loader2
} from 'lucide-react';
import { Switch, IconButton, Tooltip } from '@/component-library';
import type { ContextDocument } from '../../types';
import './DocumentItem.scss';

export interface DocumentItemProps {
  /** Document data. */
  document: ContextDocument;
  /** Toggle enabled state. */
  onToggleEnabled: (enabled: boolean) => void;
  /** Open document. */
  onOpen: () => void;
  /** Create document. */
  onCreate: (useAI: boolean) => void;
  /** Cancel creation. */
  onCancel?: () => void;
  /** Whether a document is generating. */
  isGenerating?: boolean;
}

/**
 * Document row component.
 */
export const DocumentItem: React.FC<DocumentItemProps> = ({
  document,
  onToggleEnabled,
  onOpen,
  onCreate,
  onCancel,
  isGenerating = false
}) => {
  const { t } = useTranslation('panels/project-context');
  const { name, description, exists, enabled, canGenerate } = document;

  const handleClick = useCallback(() => {
    if (exists) {
      onOpen();
    }
  }, [exists, onOpen]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && exists) {
      onOpen();
    }
  }, [exists, onOpen]);

  const handleAIGenerate = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onCreate(true);
  }, [onCreate]);

  const handleCancel = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onCancel?.();
  }, [onCancel]);

  const handleTemplateCreate = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onCreate(false);
  }, [onCreate]);

  const handleToggle = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    onToggleEnabled(e.target.checked);
  }, [onToggleEnabled]);

  return (
    <div
      className={`bitfun-document-item ${exists ? '' : 'bitfun-document-item--missing'} ${!enabled && exists ? 'bitfun-document-item--disabled' : ''}`}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role={exists ? 'button' : undefined}
      tabIndex={exists ? 0 : -1}
      // Data attributes for context menu resolution.
      data-doc-id={document.id}
      data-doc-name={document.name}
      data-doc-file-path={document.filePath || undefined}
      data-doc-exists={String(document.exists)}
      data-doc-category={document.categoryId}
    >
      <span className="bitfun-document-item__icon">
        {exists ? (
          <FileCheck size={16} />
        ) : (
          <FilePlus2 size={16} />
        )}
      </span>

      <div className="bitfun-document-item__info">
        <span className="bitfun-document-item__name">{name}</span>
        <span className="bitfun-document-item__desc">{description}</span>
      </div>

      {exists ? (
        <div className="bitfun-document-item__switch" onClick={(e) => e.stopPropagation()}>
          <Switch
            size="small"
            checked={enabled}
            onChange={handleToggle}
          />
        </div>
      ) : (
        <div className="bitfun-document-item__actions">
          {isGenerating ? (
            <>
              <Loader2 size={12} className="bitfun-document-item__loading-icon" />
              <Tooltip content={t('actions.cancel')} placement="top">
                <IconButton
                  variant="ghost"
                  size="xs"
                  onClick={handleCancel}
                  className="bitfun-document-item__cancel-btn"
                >
                  <X size={12} />
                </IconButton>
              </Tooltip>
            </>
          ) : (
            <>
              {canGenerate && (
                <Tooltip content={t('actions.aiGenerate')} placement="top">
                  <IconButton
                    variant="ai"
                    size="xs"
                    onClick={handleAIGenerate}
                  >
                    <Sparkles size={12} />
                  </IconButton>
                </Tooltip>
              )}
              <Tooltip content={t('actions.createFromTemplate')} placement="top">
                <IconButton
                  variant="ghost"
                  size="xs"
                  onClick={handleTemplateCreate}
                >
                  <Plus size={12} />
                </IconButton>
              </Tooltip>
            </>
          )}
        </div>
      )}
    </div>
  );
};

export default DocumentItem;

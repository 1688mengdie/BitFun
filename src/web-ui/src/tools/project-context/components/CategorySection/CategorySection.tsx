/**
 * CategorySection - collapsible category group.
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { 
  ChevronRight, 
  ChevronDown,
  FileText,
  Code,
  Boxes,
  GitPullRequest,
  FolderOpen,
  Import,
  Trash2,
  Bookmark,
  Database,
  Settings,
  Shield,
  Zap
} from 'lucide-react';
import { IconButton, Tooltip } from '@/component-library';
import type { ContextCategory, ContextDocument, CategoryDocuments } from '../../types';
import { DocumentItem } from '../DocumentItem';
import { MoreDocuments } from '../MoreDocuments';
import './CategorySection.scss';

// Icon map.
const ICON_MAP: Record<string, React.ReactNode> = {
  FileText: <FileText size={14} />,
  Code: <Code size={14} />,
  Boxes: <Boxes size={14} />,
  GitPullRequest: <GitPullRequest size={14} />,
  FolderOpen: <FolderOpen size={14} />,
  Bookmark: <Bookmark size={14} />,
  Database: <Database size={14} />,
  Settings: <Settings size={14} />,
  Shield: <Shield size={14} />,
  Zap: <Zap size={14} />
};

export interface CategorySectionProps {
  /** Category data. */
  category: ContextCategory;
  /** Documents under this category. */
  documents: CategoryDocuments;
  /** Whether the section is expanded. */
  isExpanded: boolean;
  /** Toggle expanded state. */
  onToggle: () => void;
  /** Double click handler. */
  onDoubleClick?: () => void;
  /** Toggle document enabled state. */
  onToggleDocument: (docId: string, enabled: boolean) => void;
  /** Open a document. */
  onOpenDocument: (doc: ContextDocument) => void;
  /** Create a document. */
  onCreateDocument: (doc: ContextDocument, useAI: boolean) => void;
  /** Cancel document creation. */
  onCancelDocument?: (docId: string) => void;
  /** Document ID currently generating. */
  generatingDocId?: string | null;
  /** Import a document into this category. */
  onImportDocument?: (categoryId: string) => void;
  /** Delete category (custom only). */
  onDeleteCategory?: (categoryId: string) => void;
}

/**
 * Collapsible category section.
 */
export const CategorySection: React.FC<CategorySectionProps> = ({
  category,
  documents,
  isExpanded,
  onToggle,
  onDoubleClick,
  onToggleDocument,
  onOpenDocument,
  onCreateDocument,
  onCancelDocument,
  generatingDocId,
  onImportDocument,
  onDeleteCategory
}) => {
  const { t } = useTranslation('panels/project-context');
  const existingCount = documents.existing.length;
  const missingCount = documents.missing.length;

  const categoryIcon = ICON_MAP[category.icon] || <FolderOpen size={14} />;

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onToggle();
    }
  }, [onToggle]);

  const handleImport = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onImportDocument?.(category.id);
  }, [category.id, onImportDocument]);

  const handleDelete = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onDeleteCategory?.(category.id);
  }, [category.id, onDeleteCategory]);

  // The general category is fixed and cannot be collapsed.
  const isGeneral = category.id === 'general';
  const canCollapse = !isGeneral;

  return (
    <div className={`bitfun-category-section ${isExpanded ? 'bitfun-category-section--expanded' : ''} ${isGeneral ? 'bitfun-category-section--fixed' : ''}`}>
      <div 
        className={`bitfun-category-section__header ${!canCollapse ? 'bitfun-category-section__header--fixed' : ''}`}
        onClick={canCollapse ? onToggle : undefined}
        onDoubleClick={canCollapse ? onDoubleClick : undefined}
        onKeyDown={canCollapse ? handleKeyDown : undefined}
        role={canCollapse ? "button" : undefined}
        tabIndex={canCollapse ? 0 : undefined}
        aria-expanded={isExpanded}
        title={category.description}
      >
        {canCollapse ? (
          <span className="bitfun-category-section__chevron">
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </span>
        ) : (
          <span className="bitfun-category-section__chevron bitfun-category-section__chevron--hidden" />
        )}
        <span className="bitfun-category-section__icon">
          {categoryIcon}
        </span>
        <span className="bitfun-category-section__name">
          {category.name}
        </span>
        {existingCount > 0 && (
          <span className="bitfun-category-section__count">
            {existingCount}
          </span>
        )}
        <div className="bitfun-category-section__actions">
          {onImportDocument && (
            <Tooltip content={t('actions.importDocument')} placement="top">
              <IconButton
                variant="ghost"
                size="xs"
                onClick={handleImport}
                className="bitfun-category-section__action-btn"
              >
                <Import size={12} />
              </IconButton>
            </Tooltip>
          )}
          {!category.isBuiltin && onDeleteCategory && (
            <Tooltip content={t('actions.deleteCategory')} placement="top">
              <IconButton
                variant="ghost"
                size="xs"
                onClick={handleDelete}
                className="bitfun-category-section__action-btn bitfun-category-section__action-btn--delete"
              >
                <Trash2 size={12} />
              </IconButton>
            </Tooltip>
          )}
        </div>
      </div>

      {isExpanded && (
        <div className="bitfun-category-section__content">
          <div className="bitfun-category-section__content-inner">
            {documents.existing.length > 0 && (
              <div className="bitfun-category-section__group">
                {documents.existing.map(doc => (
                  <DocumentItem
                    key={doc.id}
                    document={doc}
                    onToggleEnabled={(enabled) => onToggleDocument(doc.id, enabled)}
                    onOpen={() => onOpenDocument(doc)}
                    onCreate={(useAI) => onCreateDocument(doc, useAI)}
                    onCancel={onCancelDocument ? () => onCancelDocument(doc.id) : undefined}
                    isGenerating={generatingDocId === doc.id}
                  />
                ))}
              </div>
            )}

            {documents.missing.length > 0 && (
              <MoreDocuments
                documents={documents.missing}
                onToggleDocument={onToggleDocument}
                onOpenDocument={onOpenDocument}
                onCreateDocument={onCreateDocument}
                onCancelDocument={onCancelDocument}
                generatingDocId={generatingDocId}
              />
            )}

            {existingCount === 0 && missingCount === 0 && (
              <div className="bitfun-category-section__empty">
                <span>{t('category.noDocuments')}</span>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default CategorySection;

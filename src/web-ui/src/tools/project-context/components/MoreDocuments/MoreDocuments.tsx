/**
 * MoreDocuments expandable list component.
 */

import React, { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { ContextDocument } from '../../types';
import { DocumentItem } from '../DocumentItem';
import './MoreDocuments.scss';

export interface MoreDocumentsProps {
  /** Document list. */
  documents: ContextDocument[];
  /** Toggle document enabled state. */
  onToggleDocument: (docId: string, enabled: boolean) => void;
  /** Open a document. */
  onOpenDocument: (doc: ContextDocument) => void;
  /** Create a document. */
  onCreateDocument: (doc: ContextDocument, useAI: boolean) => void;
  /** Cancel document creation. */
  onCancelDocument?: (docId: string) => void;
  /** Document id currently generating. */
  generatingDocId?: string | null;
}

export const MoreDocuments: React.FC<MoreDocumentsProps> = ({
  documents,
  onToggleDocument,
  onOpenDocument,
  onCreateDocument,
  onCancelDocument,
  generatingDocId
}) => {
  const { t } = useTranslation('panels/project-context');
  const [isExpanded, setIsExpanded] = useState(false);

  const handleToggle = useCallback(() => {
    setIsExpanded(prev => !prev);
  }, []);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleToggle();
    }
  }, [handleToggle]);

  if (documents.length === 0) {
    return null;
  }

  return (
    <div className={`bitfun-more-documents ${isExpanded ? 'bitfun-more-documents--expanded' : ''}`}>
      <div 
        className="bitfun-more-documents__trigger"
        onClick={handleToggle}
        onKeyDown={handleKeyDown}
        role="button"
        tabIndex={0}
        aria-expanded={isExpanded}
      >
        <span className="bitfun-more-documents__line" />
        <span className="bitfun-more-documents__text">
          {isExpanded ? t('moreDocuments.collapse') : t('moreDocuments.expand')}
        </span>
        <span className="bitfun-more-documents__line" />
      </div>

      {isExpanded && (
        <div className="bitfun-more-documents__list">
          {documents.map(doc => (
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
    </div>
  );
};

export default MoreDocuments;

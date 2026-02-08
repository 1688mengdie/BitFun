/**
 * Git diff editor wrapper around `DiffEditor`.
 * Adds Git-oriented actions (accept/reject) and optional dirty-state tracking.
 */

import React, { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { DiffEditor } from '@/tools/editor';
import { GitDiffService, DiffStats } from '../../services/gitDiffService';
import { Check, X } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import './GitDiffEditor.scss';

const log = createLogger('GitDiffEditor');

export interface GitDiffEditorProps {
  /** Original content (HEAD version) */
  originalContent: string;
  /** Modified content (working directory version) */
  modifiedContent: string;
  /** File path */
  filePath: string;
  /** Repository path */
  repositoryPath: string;
  /** Language */
  language?: string;
  /** Callback after accepting all changes */
  onAcceptAll?: () => void;
  /** Callback after rejecting all changes */
  onRejectAll?: () => void;
  /** Close callback */
  onClose?: () => void;
  /** Content change callback (for dirty state tracking) */
  onContentChange?: (content: string, hasChanges: boolean) => void;
  /** Save callback */
  onSave?: (content: string) => void;
}

export const GitDiffEditor: React.FC<GitDiffEditorProps> = ({
  originalContent,
  modifiedContent,
  filePath,
  repositoryPath,
  language,
  onAcceptAll,
  onRejectAll,
  onClose,
  onContentChange,
  onSave
}) => {
  const { t } = useTranslation('panels/git');
  const [isProcessing, setIsProcessing] = useState(false);
  const [isCompleted, setIsCompleted] = useState(false);
  const [operationResult, setOperationResult] = useState<'accepted' | 'rejected' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [currentModifiedContent, setCurrentModifiedContent] = useState(modifiedContent);
  const [hasChanges, setHasChanges] = useState(false);
  const [saving, setSaving] = useState(false);
  const [lastSavedContent, setLastSavedContent] = useState(modifiedContent);


  const hasChangesRef = useRef<boolean>(false);
  const currentModifiedContentRef = useRef<string>(modifiedContent);
  const lastSavedContentRef = useRef<string>(modifiedContent);
  const saveFileContentRef = useRef<() => Promise<void>>();
  

  useEffect(() => {
    hasChangesRef.current = hasChanges;
  }, [hasChanges]);
  
  useEffect(() => {
    currentModifiedContentRef.current = currentModifiedContent;
  }, [currentModifiedContent]);

  useEffect(() => {
    lastSavedContentRef.current = lastSavedContent;
  }, [lastSavedContent]);


  const diffStats: DiffStats = useMemo(() => {

    return GitDiffService.calculateStatsSync(originalContent, currentModifiedContent);
  }, [originalContent, currentModifiedContent]);
  

  const saveFileContent = useCallback(async () => {
    if (!filePath || !repositoryPath) {
      log.warn('Missing required parameters, skipping save', { filePath, repositoryPath });
      return;
    }


    const currentHasChanges = hasChangesRef.current;
    const contentToSave = currentModifiedContentRef.current;

    if (!currentHasChanges) {
      return;
    }

    setSaving(true);
    setError(null);

    try {
      const { workspaceAPI } = await import('@/infrastructure/api');


      await workspaceAPI.writeFileContent(repositoryPath, filePath, contentToSave);


      setLastSavedContent(contentToSave);
      lastSavedContentRef.current = contentToSave;


      setHasChanges(false);
      hasChangesRef.current = false;


      onContentChange?.(contentToSave, false);
      onSave?.(contentToSave);

    } catch (err) {
      log.error('Failed to save file', { filePath, repositoryPath, error: err });
      setError(t('diffEditor.saveFailedWithMessage', { error: String(err) }));
    } finally {
      setSaving(false);
    }
  }, [filePath, repositoryPath, onContentChange, onSave, t]);
  

  useEffect(() => {
    saveFileContentRef.current = saveFileContent;
  }, [saveFileContent]);
  

  const handleModifiedContentChange = useCallback((content: string) => {
    setCurrentModifiedContent(content);


    const isModified = content !== lastSavedContentRef.current;

    setHasChanges(isModified);
    hasChangesRef.current = isModified;


    onContentChange?.(content, isModified);
  }, [onContentChange]);
  

  const handleContainerKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if ((event.ctrlKey || event.metaKey) && event.key === 's') {
      event.preventDefault();
      event.stopPropagation();
      
      saveFileContentRef.current?.();
    }
  }, []);


  const handleAcceptAll = useCallback(async () => {
    if (isProcessing) return;


    const confirmed = await window.confirm(t('diffEditor.confirmAcceptAll', {
      filePath,
      additions: diffStats.additions,
      deletions: diffStats.deletions
    }));

    if (!confirmed) return;

    setIsProcessing(true);
    setError(null);

    try {

      if (currentModifiedContent !== modifiedContent) {
        const { workspaceAPI } = await import('@/infrastructure/api');

        await workspaceAPI.writeFileContent(repositoryPath, filePath, currentModifiedContent);
      }

      await GitDiffService.acceptAllChanges(repositoryPath, filePath);
      
      setOperationResult('accepted');
      setIsCompleted(true);
      

      onAcceptAll?.();
      
      setTimeout(() => {
        onClose?.();
      }, 2000);
      
    } catch (err) {
      log.error('Failed to accept changes', { filePath, repositoryPath, error: err });
      setError(t('diffEditor.acceptFailedWithMessage', { error: String(err) }));
    } finally {
      setIsProcessing(false);
    }
  }, [
    isProcessing,
    filePath,
    repositoryPath,
    currentModifiedContent,
    modifiedContent,
    diffStats,
    onAcceptAll,
    onClose,
    t
  ]);


  const handleRejectAll = useCallback(async () => {
    if (isProcessing) return;


    const confirmed = await window.confirm(t('diffEditor.confirmRejectAll', {
      filePath,
      changes: diffStats.changes
    }));

    if (!confirmed) return;

    setIsProcessing(true);
    setError(null);

    try {
      await GitDiffService.rejectAllChanges(repositoryPath, filePath);
      
      setOperationResult('rejected');
      setIsCompleted(true);
      

      onRejectAll?.();
      
      setTimeout(() => {
        onClose?.();
      }, 2000);
      
    } catch (err) {
      log.error('Failed to reject changes', { filePath, repositoryPath, error: err });
      setError(t('diffEditor.rejectFailedWithMessage', { error: String(err) }));
    } finally {
      setIsProcessing(false);
    }
  }, [isProcessing, filePath, repositoryPath, diffStats, onRejectAll, onClose, t]);


  if (isCompleted) {
    return (
      <div className="git-diff-editor git-diff-editor--completed">
        <div className="git-diff-editor__completed-message">
          <div className={`git-diff-editor__completed-icon git-diff-editor__completed-icon--${operationResult}`}>
            {operationResult === 'accepted' ? <Check size={48} /> : <X size={48} />}
          </div>
          
          <h2 className="git-diff-editor__completed-title">
            {operationResult === 'accepted'
              ? t('diffEditor.completed.acceptedTitle')
              : t('diffEditor.completed.rejectedTitle')
            }
          </h2>
          
          <div className="git-diff-editor__completed-details">
            <p className="git-diff-editor__file-path">{filePath}</p>
            
            {operationResult === 'accepted' ? (
              <p className="git-diff-editor__result-text git-diff-editor__result-text--success">
                {t('diffEditor.completed.stagedHint')}
              </p>
            ) : (
              <p className="git-diff-editor__result-text git-diff-editor__result-text--warning">
                {t('diffEditor.completed.rejectedHint')}
              </p>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div 
      className="git-diff-editor"
      onKeyDownCapture={handleContainerKeyDown}
    >
      {error && (
        <div className="git-diff-editor__error">
          <X size={16} />
          <span>{error}</span>
        </div>
      )}

      <div className="git-diff-editor__content">
        <DiffEditor
          key={`diff-${filePath}-${originalContent.length}`}
          originalContent={originalContent}
          modifiedContent={currentModifiedContent}
          filePath={filePath}
          repositoryPath={repositoryPath}
          language={language}
          renderSideBySide={true}
          readOnly={false}
          showMinimap={false}
          renderIndicators={false}
          onModifiedContentChange={handleModifiedContentChange}
          onSave={saveFileContent}
        />
      </div>

      {saving && (
        <div className="git-diff-editor__saving-indicator">
          {t('common.saving')}
        </div>
      )}

      {isProcessing && (
        <div className="git-diff-editor__processing-overlay">
          <div className="git-diff-editor__spinner" />
          <p>{t('common.processing')}</p>
        </div>
      )}
    </div>
  );
};

export default GitDiffEditor;


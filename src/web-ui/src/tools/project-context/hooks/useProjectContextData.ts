/**
 * useProjectContextData manages project context data.
 * Fetches document status, category state, and document actions.
 */

import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';
import {
  ContextCategory,
  ContextDocument,
  CategoryDocuments,
  CategoryId,
  BUILTIN_CATEGORIES
} from '../types';

const log = createLogger('useProjectContextData');

// Backend category payload
interface BackendCategoryInfo {
  id: string;
  name: string;
  description?: string;
  icon: string;
  isBuiltin: boolean;
  order: number;
}

export interface UseProjectContextDataOptions {
  /** Workspace path */
  workspacePath: string;
}

export interface UseProjectContextDataReturn {
  /** Loading state */
  loading: boolean;
  /** Category list */
  categories: ContextCategory[];
  /** Documents under a category */
  getCategoryDocuments: (categoryId: CategoryId) => CategoryDocuments;
  /** Expanded category IDs */
  expandedCategories: Set<CategoryId>;
  /** Toggle category expanded state */
  toggleCategory: (categoryId: CategoryId) => void;
  /** Toggle document enabled state */
  toggleDocument: (docId: string, enabled: boolean) => void;
  /** Open a document */
  openDocument: (doc: ContextDocument) => Promise<void>;
  /** Create a document */
  createDocument: (doc: ContextDocument, useAI: boolean) => Promise<void>;
  /** Cancel document creation */
  cancelDocument: (docId: string) => void;
  /** Document ID currently generating */
  generatingDocId: string | null;
  /** Refresh data */
  refresh: () => Promise<void>;
  /** Add a custom category */
  addCustomCategory: (name: string, icon: string, description?: string) => Promise<void>;
  /** Delete a custom category */
  deleteCategory: (categoryId: CategoryId) => Promise<void>;
  /** Import a document */
  importDocument: (categoryId: CategoryId, filePath: string) => Promise<void>;
  /** Delete a document */
  deleteDocument: (docId: string) => Promise<void>;
}

export function useProjectContextData({
  workspacePath
}: UseProjectContextDataOptions): UseProjectContextDataReturn {
  const { t } = useTranslation('panels/project-context');
  
  const [loading, setLoading] = useState(true);
  
  const [documents, setDocuments] = useState<ContextDocument[]>([]);
  
  // "General" stays expanded and cannot be collapsed.
  const [expandedCategories, setExpandedCategories] = useState<Set<CategoryId>>(
    new Set(['general'])
  );
  
  const [customCategories, setCustomCategories] = useState<ContextCategory[]>([]);
  
  const [generatingDocId, setGeneratingDocId] = useState<string | null>(null);
  const cancelGenerationRef = useRef(false);

  // Merge categories and translate built-in names/descriptions.
  const categories = useMemo(() => {
    const translatedBuiltinCategories = BUILTIN_CATEGORIES.map(cat => ({
      ...cat,
      // Built-in name/description are i18n keys.
      name: cat.isBuiltin ? t(cat.name) : cat.name,
      description: cat.isBuiltin && cat.description ? t(cat.description) : cat.description
    }));
    return [...translatedBuiltinCategories, ...customCategories].sort((a, b) => a.order - b.order);
  }, [customCategories, t]);

  // Fetch document status and categories from the backend.
  const fetchDocuments = useCallback(async () => {
    if (!workspacePath) {
      setLoading(false);
      return;
    }

    setLoading(true);
    try {
      const [backendDocs, backendCategories] = await Promise.all([
        invoke<ContextDocument[]>('get_document_statuses', { workspacePath }),
        invoke<BackendCategoryInfo[]>('get_all_categories', { workspacePath })
      ]);

      const docsWithState: ContextDocument[] = backendDocs.map(doc => ({
        ...doc,
        isCustom: false // Backend documents are built-in.
      }));

      setDocuments(docsWithState);

      const frontendCategories: ContextCategory[] = backendCategories.map(cat => {
        if (cat.isBuiltin) {
          // Built-in: read i18n keys and icons from BUILTIN_CATEGORIES.
          const builtinCat = BUILTIN_CATEGORIES.find(bc => bc.id === cat.id);
          return {
            id: cat.id,
            name: builtinCat?.name || cat.name,
            icon: builtinCat?.icon || cat.icon || 'FolderOpen',
            description: builtinCat?.description || cat.description || '',
            isBuiltin: true,
            order: cat.order
          };
        } else {
          // Custom categories keep backend-provided icons.
          return {
            id: cat.id,
            name: cat.name,
            icon: cat.icon || 'FolderOpen',
            description: cat.description || t('category.customDescription'),
            isBuiltin: false,
            order: cat.order
          };
        }
      });

      setCustomCategories(frontendCategories.filter(cat => !cat.isBuiltin));
    } catch (err) {
      log.error('Failed to get document state', { workspacePath, error: err });
    } finally {
      setLoading(false);
    }
  }, [workspacePath, t]);

  // Build category docs and map descriptions by doc id.
  const getCategoryDocuments = useCallback((categoryId: CategoryId): CategoryDocuments => {
    const categoryDocs = documents.filter(doc => doc.categoryId === categoryId);

    const translateDoc = (doc: ContextDocument): ContextDocument => ({
      ...doc,
      description: !doc.isCustom ? t(`builtinDocuments.${doc.id}`) : doc.description
    });

    // Missing docs are ordered by priority: high > medium > low.
    const priorityOrder = { high: 0, medium: 1, low: 2 };
    const missingDocs = categoryDocs
      .filter(doc => !doc.exists)
      .map(translateDoc)
      .sort((a, b) => priorityOrder[a.priority] - priorityOrder[b.priority]);

    return {
      existing: categoryDocs.filter(doc => doc.exists).map(translateDoc),
      missing: missingDocs
    };
  }, [documents, t]);

  const toggleCategory = useCallback((categoryId: CategoryId) => {
    // "General" cannot be collapsed.
    if (categoryId === 'general') return;
    
    setExpandedCategories(prev => {
      const newSet = new Set(prev);
      if (newSet.has(categoryId)) {
        newSet.delete(categoryId);
      } else {
        newSet.add(categoryId);
      }
      return newSet;
    });
  }, []);

  const toggleDocument = useCallback(async (docId: string, enabled: boolean) => {
    setDocuments(prev => prev.map(d =>
      d.id === docId ? { ...d, enabled } : d
    ));

    try {
      await invoke('toggle_document_enabled', {
        workspacePath,
        docId,
        enabled
      });
    } catch (err) {
      log.error('Failed to toggle document state', { workspacePath, docId, enabled, error: err });
      setDocuments(prev => prev.map(d =>
        d.id === docId ? { ...d, enabled: !enabled } : d
      ));
    }
  }, [documents, workspacePath]);

  const openDocument = useCallback(async (doc: ContextDocument) => {
    if (!doc.exists || !doc.filePath) return;
    
    try {
      const { fileTabManager } = await import('@/shared/services/FileTabManager');
      fileTabManager.openFileAndJump(doc.filePath, 1, 1, { workspacePath });
    } catch (err) {
      log.error('Failed to open document', { workspacePath, filePath: doc.filePath, error: err });
    }
  }, [workspacePath]);

  const createDocument = useCallback(async (doc: ContextDocument, useAI: boolean) => {
    if (!doc.filePath) return;

    setGeneratingDocId(doc.id);
    cancelGenerationRef.current = false;
    try {
      if (useAI) {
        if (!doc.canGenerate) {
          throw new Error('This document does not support AI generation');
        }

        const generatedPath = await invoke<string>('generate_context_document', {
          workspacePath,
          docId: doc.id
        });

        // Backend returns a sentinel when generation is canceled.
        if (generatedPath === '__CANCELLED__' || cancelGenerationRef.current) {
          await fetchDocuments();
          return;
        }

        await fetchDocuments();

        const { fileTabManager } = await import('@/shared/services/FileTabManager');
        fileTabManager.openFileAndJump(generatedPath, 1, 1, { workspacePath });
      } else {
        const createdPath = await invoke<string>('create_context_document', {
          workspacePath,
          docId: doc.id
        });

        await fetchDocuments();

        const { fileTabManager } = await import('@/shared/services/FileTabManager');
        fileTabManager.openFileAndJump(createdPath, 1, 1, { workspacePath });
      }
    } catch (err) {
      log.error('Failed to create document', { workspacePath, docId: doc.id, useAI, error: err });
      const errorMessage = err instanceof Error ? err.message : String(err);
      alert(`Failed to create document: ${errorMessage}`);
    } finally {
      setGeneratingDocId(null);
    }
  }, [workspacePath, fetchDocuments]);

  const cancelDocument = useCallback(async (docId: string) => {
    if (generatingDocId === docId) {
      try {
        await invoke('cancel_context_document_generation', {
          workspacePath,
          docId
        });
      } catch (err) {
        log.error('Failed to cancel document generation', { workspacePath, docId, error: err });
      } finally {
        // Clear local state regardless of backend result.
        cancelGenerationRef.current = true;
        setGeneratingDocId(null);
      }
    }
  }, [generatingDocId, workspacePath]);

  const addCustomCategory = useCallback(async (name: string, icon: string, description?: string) => {
    try {
      const categoryId = await invoke<string>('create_project_category', {
        workspacePath,
        name,
        description,
        icon
      });

      await fetchDocuments();

      setExpandedCategories(prev => new Set([...prev, categoryId]));
    } catch (err) {
      log.error('Failed to create category', { workspacePath, name, icon, error: err });
      throw err;
    }
  }, [workspacePath, fetchDocuments]);

  const deleteCategory = useCallback(async (categoryId: CategoryId) => {
    try {
      await invoke('delete_project_category', {
        workspacePath,
        categoryId
      });

      await fetchDocuments();

      setExpandedCategories(prev => {
        const newSet = new Set(prev);
        newSet.delete(categoryId);
        return newSet;
      });
    } catch (err) {
      log.error('Failed to delete category', { workspacePath, categoryId, error: err });
      throw err;
    }
  }, [workspacePath, fetchDocuments]);

  const importDocument = useCallback(async (categoryId: CategoryId, sourcePath: string) => {
    if (!workspacePath) {
      log.error('workspacePath is undefined');
      return;
    }

    const fileName = sourcePath.split(/[/\\]/).pop() || 'unknown.md';

    // Only .md files are supported.
    if (!fileName.toLowerCase().endsWith('.md')) {
      log.error('Only .md files are supported', { sourcePath });
      return;
    }

    try {
      const importedDoc = await invoke<{
        id: string;
        name: string;
        file_path: string;
      }>('import_project_document', {
        workspacePath,
        sourcePath,
        name: fileName,
        categoryId,
        priority: 'high',
        onConflict: 'rename' // Default: auto-rename
      });

      await fetchDocuments();

      const { fileTabManager } = await import('@/shared/services/FileTabManager');
      fileTabManager.openFileAndJump(importedDoc.file_path, 1, 1, { workspacePath });
    } catch (err) {
      log.error('Failed to import document', { workspacePath, categoryId, sourcePath, error: err });
      throw err;
    }
  }, [workspacePath, fetchDocuments]);

  const deleteDocument = useCallback(async (docId: string) => {
    try {
      await invoke('delete_context_document', {
        workspacePath,
        docId
      });

      await fetchDocuments();
    } catch (err) {
      log.error('Failed to delete document', { workspacePath, docId, error: err });
      throw err;
    }
  }, [workspacePath, fetchDocuments]);

  const refresh = useCallback(async () => {
    await fetchDocuments();
  }, [fetchDocuments]);

  useEffect(() => {
    fetchDocuments();
  }, [fetchDocuments]);

  return {
    loading,
    categories,
    getCategoryDocuments,
    expandedCategories,
    toggleCategory,
    toggleDocument,
    openDocument,
    createDocument,
    cancelDocument,
    generatingDocId,
    refresh,
    addCustomCategory,
    deleteCategory,
    importDocument,
    deleteDocument
  };
}

export default useProjectContextData;

/**
 * Project Context panel (refactor).
 * Tabs include docs (categorized view) and knowledge.
 * The docs tab merges the previous "docs" and "specs".
 */

import React, { useState, useCallback, useMemo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ScrollText,
  BookOpen,
  RefreshCw,
  Plus,
  Cpu
} from 'lucide-react';
import { PanelHeader } from '@/app/components/panels/base';
import { IconButton, Button, ConfirmDialog, Tabs, TabPane } from '@/component-library';
import { ProjectContextPanelProps } from '../../types';
import { useProjectContextData } from '../../hooks/useProjectContextData';
import { CategorySection } from '../CategorySection';
import { KnowledgeTab } from '../KnowledgeTab';
import { AddCategoryDialog } from '../AddCategoryDialog';
import { globalEventBus } from '@/infrastructure/event-bus';
import { getDefaultPrimaryModel } from '@/infrastructure/config/utils/modelConfigHelpers';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';
import './ProjectContextPanel.scss';

const log = createLogger('ProjectContextPanel');

type TabType = 'docs' | 'knowledge';

const ProjectContextPanel: React.FC<ProjectContextPanelProps> = ({
  workspacePath,
  isActive = false,
  onActivate,
  className = ''
}) => {
  const { t } = useTranslation('panels/project-context');
  
  const [activeTab, setActiveTab] = useState<TabType>('docs');
  
  const [showAddCategoryDialog, setShowAddCategoryDialog] = useState(false);
  
  const [categoryToDelete, setCategoryToDelete] = useState<string | null>(null);

  const [documentToDelete, setDocumentToDelete] = useState<{ id: string; name: string } | null>(null);

  const {
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
  } = useProjectContextData({ workspacePath });

  // Context window for token usage display
  const [contextWindow, setContextWindow] = useState<number | null>(null);

  useEffect(() => {
    const fetchContextWindow = async () => {
      try {
        const modelId = await getDefaultPrimaryModel();
        if (modelId) {
          const models = await configManager.getConfig<any[]>('ai.models') || [];
          const model = models.find((m: any) => m.id === modelId);
          if (model?.context_window) {
            setContextWindow(model.context_window);
          }
        }
      } catch (err) {
        log.warn('Failed to get context window', err);
      }
    };
    fetchContextWindow();
  }, []);

  // Total tokens from enabled documents (real data)
  const docsTokenCount = useMemo(() => {
    let total = 0;
    categories.forEach(cat => {
      const docs = getCategoryDocuments(cat.id);
      docs.existing.forEach(doc => {
        if (doc.enabled) {
          total += doc.tokenCount || 0;
        }
      });
    });
    return total;
  }, [categories, getCategoryDocuments]);

  const totalTokens = docsTokenCount;

  // Usage percent when context window is available
  const usagePercent = useMemo(() => {
    if (contextWindow === null) return '0';
    return ((totalTokens / contextWindow) * 100).toFixed(1);
  }, [totalTokens, contextWindow]);

  const formatTokenCount = (count: number): string => {
    if (count >= 1000) {
      return `${(count / 1000).toFixed(1)}k`;
    }
    return count.toString();
  };

  const handleRefresh = useCallback(async () => {
    try {
      await refresh();
    } catch (err) {
      log.error('Failed to refresh', err);
    }
  }, [refresh]);

  const handleAddCategory = useCallback(async (name: string, icon: string, description?: string) => {
    try {
      await addCustomCategory(name, icon, description);
    } catch (err) {
      log.error('Failed to add category', { name, error: err });
    }
  }, [addCustomCategory]);

  const handleImportDocument = useCallback(async (categoryId: string) => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [
          { name: 'Markdown Files', extensions: ['md'] }
        ],
        title: t('actions.selectMarkdownFile')
      });
      
      if (selected && typeof selected === 'string') {
        // Copies file to .bitfun/docs/{category_id}/
        await importDocument(categoryId, selected);
      }
    } catch (err) {
      log.error('Failed to import document', { categoryId, error: err });
    }
  }, [importDocument]);

  const handleDeleteCategory = useCallback((categoryId: string) => {
    setCategoryToDelete(categoryId);
  }, []);

  const confirmDeleteCategory = useCallback(async () => {
    if (categoryToDelete) {
      try {
        await deleteCategory(categoryToDelete);
        setCategoryToDelete(null);
      } catch (err) {
        log.error('Failed to delete category', { categoryId: categoryToDelete, error: err });
      }
    }
  }, [categoryToDelete, deleteCategory]);

  const cancelDeleteCategory = useCallback(() => {
    setCategoryToDelete(null);
  }, []);

  // Handle delete events from the context menu
  useEffect(() => {
    const handleDeleteDocument = (data: { docId: string; docName: string }) => {
      setDocumentToDelete({ id: data.docId, name: data.docName });
    };

    globalEventBus.on('project-context:delete-document', handleDeleteDocument);

    return () => {
      globalEventBus.off('project-context:delete-document', handleDeleteDocument);
    };
  }, []);

  const confirmDeleteDocument = useCallback(async () => {
    if (documentToDelete) {
      try {
        await deleteDocument(documentToDelete.id);
        setDocumentToDelete(null);
      } catch (err) {
        log.error('Failed to delete document', { docId: documentToDelete.id, error: err });
      }
    }
  }, [documentToDelete, deleteDocument]);

  const cancelDeleteDocument = useCallback(() => {
    setDocumentToDelete(null);
  }, []);

  const expandedCount = expandedCategories.size;

  const collapseAllCategories = useCallback(() => {
    categories.forEach(cat => {
      if (expandedCategories.has(cat.id)) {
        toggleCategory(cat.id);
      }
    });
  }, [categories, expandedCategories, toggleCategory]);

  const expandOnlyCategory = useCallback((categoryId: string) => {
    categories.forEach(cat => {
      const isExpanded = expandedCategories.has(cat.id);
      if (cat.id === categoryId && !isExpanded) {
        toggleCategory(cat.id);
      } else if (cat.id !== categoryId && isExpanded) {
        toggleCategory(cat.id);
      }
    });
  }, [categories, expandedCategories, toggleCategory]);

  // Double-click behavior: keep one open or collapse all
  const handleDoubleClick = useCallback((categoryId: string) => {
    if (expandedCount > 1) {
      expandOnlyCategory(categoryId);
    } else if (expandedCount === 1 && expandedCategories.has(categoryId)) {
      collapseAllCategories();
    }
  }, [expandedCount, expandedCategories, expandOnlyCategory, collapseAllCategories]);


  const renderDocsContent = () => {
    if (loading) {
      return (
        <div className="bitfun-project-context__loading">
          <RefreshCw size={16} className="bitfun-spin" />
          <span>{t('loading.scanning')}</span>
        </div>
      );
    }

    return (
      <div className="bitfun-project-context__docs-view">
        <div className="bitfun-project-context__categories">
          {categories.map(category => (
            <CategorySection
              key={category.id}
              category={category}
              documents={getCategoryDocuments(category.id)}
              isExpanded={expandedCategories.has(category.id)}
              onToggle={() => toggleCategory(category.id)}
              onDoubleClick={() => handleDoubleClick(category.id)}
              onToggleDocument={toggleDocument}
              onOpenDocument={openDocument}
              onCreateDocument={createDocument}
              onCancelDocument={cancelDocument}
              generatingDocId={generatingDocId}
              onImportDocument={handleImportDocument}
              onDeleteCategory={!category.isBuiltin ? handleDeleteCategory : undefined}
            />
          ))}
          
          <Button 
            variant="ghost"
            size="small"
            className="bitfun-project-context__add-category-btn"
            onClick={() => setShowAddCategoryDialog(true)}
            title={t('category.addCategoryTitle')}
          >
            <Plus size={12} />
            <span>{t('category.addCategory')}</span>
          </Button>
        </div>

        <AddCategoryDialog
          isOpen={showAddCategoryDialog}
          onClose={() => setShowAddCategoryDialog(false)}
          onConfirm={handleAddCategory}
        />

        <ConfirmDialog
          isOpen={!!categoryToDelete}
          onClose={cancelDeleteCategory}
          onConfirm={confirmDeleteCategory}
          title={t('dialog.deleteCategory.title', 'Delete category')}
          message={
            <>
              <p>{t('dialog.deleteCategory.confirm')}</p>
              <span style={{ color: 'var(--color-text-muted)', fontSize: '12px' }}>{t('dialog.deleteCategory.hint')}</span>
            </>
          }
          type="warning"
          confirmText={t('dialog.deleteCategory.delete')}
          cancelText={t('dialog.deleteCategory.cancel')}
          confirmDanger
        />

        <ConfirmDialog
          isOpen={!!documentToDelete}
          onClose={cancelDeleteDocument}
          onConfirm={confirmDeleteDocument}
          title={t('dialog.deleteDocument.title', 'Delete document')}
          message={t('dialog.deleteDocument.confirm', { name: documentToDelete?.name || '' })}
          type="warning"
          confirmText={t('dialog.deleteDocument.delete')}
          cancelText={t('dialog.deleteDocument.cancel')}
          confirmDanger
        />
      </div>
    );
  };

  return (
    <div
      className={`bitfun-project-context ${isActive ? 'bitfun-project-context--active' : ''} ${className}`}
      onClick={onActivate}
    >
      <PanelHeader
        title={t('title')}
        actions={
          <IconButton
            size="xs"
            onClick={(e) => {
              e.stopPropagation();
              handleRefresh();
            }}
            tooltip={t('actions.refresh')}
          >
            <RefreshCw size={14} />
          </IconButton>
        }
      />

      <Tabs
        activeKey={activeTab}
        onChange={(key) => setActiveTab(key as TabType)}
        type="line"
        size="small"
        className="bitfun-project-context__tabs"
      >
        <TabPane
          tabKey="docs"
          label={t('tabs.docs')}
          icon={<ScrollText size={14} />}
        >
          {renderDocsContent()}
        </TabPane>
        <TabPane
          tabKey="knowledge"
          label={t('tabs.knowledge')}
          icon={<BookOpen size={14} />}
        >
          <KnowledgeTab workspacePath={workspacePath} />
        </TabPane>
      </Tabs>

      <div className="bitfun-project-context__token-bar">
        <div className="bitfun-project-context__token-bar-compact">
          <div className="bitfun-project-context__token-bar-icon">
            <Cpu size={11} />
          </div>
          <div className="bitfun-project-context__token-bar-stats">
            <span className="bitfun-project-context__token-bar-value">
              {formatTokenCount(totalTokens)}
            </span>
            {contextWindow !== null && (
              <>
                <span className="bitfun-project-context__token-bar-separator">/</span>
                <span className="bitfun-project-context__token-bar-max">
                  {formatTokenCount(contextWindow)}
                </span>
                <span className="bitfun-project-context__token-bar-percent">
                  ({usagePercent}%)
                </span>
              </>
            )}
          </div>
          {contextWindow !== null && (
            <div className="bitfun-project-context__token-bar-progress">
              <div
                className="bitfun-project-context__token-bar-progress-fill"
                style={{ width: `${Math.min((totalTokens / contextWindow) * 100, 100)}%` }}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default ProjectContextPanel;

/**
 * Knowledge tab component for Skill/RAG management.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { 
  BookOpen,
  Plus,
  RefreshCw,
  ChevronDown,
  ChevronRight,
  Brain,
  Database
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('KnowledgeTab');
import { IconButton, Tooltip, Search, ConfirmDialog } from '@/component-library';
import type { 
  KnowledgeBaseItem, 
  KnowledgeBaseType,
  AddKnowledgeFormData 
} from '../../types/knowledge';
import { KnowledgeItem } from '../KnowledgeItem';
import { AddKnowledgeDialog } from '../AddKnowledgeDialog';
import './KnowledgeTab.scss';

export interface KnowledgeTabProps {
  workspacePath: string;
}

export const KnowledgeTab: React.FC<KnowledgeTabProps> = ({ workspacePath }) => {
  const { t } = useTranslation('panels/project-context');
  
  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBaseItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  
  const [skillExpanded, setSkillExpanded] = useState(true);
  const [ragExpanded, setRagExpanded] = useState(true);
  
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null);

  const loadKnowledgeBases = useCallback(async () => {
    if (!workspacePath) {
      setLoading(false);
      return;
    }

    setLoading(true);
    try {
      const result = await invoke<KnowledgeBaseItem[]>('get_knowledge_bases', {
        workspacePath
      }).catch(() => []);
      
      setKnowledgeBases(result);
    } catch (err) {
      log.error('Failed to load knowledge bases', { workspacePath, error: err });
      // Fallback to mock data for demo use.
      setKnowledgeBases(getMockData());
    } finally {
      setLoading(false);
    }
  }, [workspacePath]);

  // Mock data for demo usage.
  const getMockData = (): KnowledgeBaseItem[] => [
    {
      id: 'skill-1',
      name: 'React Best Practices',
      description: 'React development standards and best practices guide',
      type: 'skill',
      icon: 'Code',
      enabled: true,
      status: 'active',
      tags: ['react', 'frontend'],
      tokenEstimate: 2500,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      config: {
        filePath: '/docs/react-best-practices.md',
        format: 'markdown',
        autoSync: true
      }
    },
    {
      id: 'skill-2',
      name: 'TypeScript Guidelines',
      description: 'TypeScript coding guidelines',
      type: 'skill',
      icon: 'FileText',
      enabled: true,
      status: 'active',
      tags: ['typescript'],
      tokenEstimate: 1800,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      config: {
        content: '# TypeScript Guidelines\n\n...',
        format: 'markdown',
        autoSync: false
      }
    },
    {
      id: 'rag-1',
      name: 'API Documentation Library',
      description: 'Project API documentation search',
      type: 'rag',
      icon: 'Database',
      enabled: true,
      status: 'active',
      tags: ['api', 'docs'],
      tokenEstimate: 0,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      config: {
        endpoint: 'https://api.example.com/search',
        maxResults: 5,
        similarityThreshold: 0.7
      }
    }
  ];

  const toggleKnowledgeEnabled = useCallback(async (id: string) => {
    const kb = knowledgeBases.find(k => k.id === id);
    if (!kb) return;

    const newEnabled = !kb.enabled;
    
    setKnowledgeBases(prev => prev.map(k => 
      k.id === id ? { ...k, enabled: newEnabled } : k
    ));

    try {
      await invoke('update_knowledge_base', {
        workspacePath,
        knowledgeId: id,
        updates: { enabled: newEnabled }
      });
    } catch (err) {
      log.error('Failed to update knowledge base', { workspacePath, knowledgeId: id, enabled: newEnabled, error: err });
      setKnowledgeBases(prev => prev.map(k => 
        k.id === id ? { ...k, enabled: !newEnabled } : k
      ));
    }
  }, [knowledgeBases, workspacePath]);

  const deleteKnowledge = useCallback(async (id: string) => {
    try {
      await invoke('delete_knowledge_base', {
        workspacePath,
        knowledgeId: id
      }).catch(() => {});
      
      setKnowledgeBases(prev => prev.filter(k => k.id !== id));
      setShowDeleteConfirm(null);
    } catch (err) {
      log.error('Failed to delete knowledge base', { workspacePath, knowledgeId: id, error: err });
    }
  }, [workspacePath]);

  const syncKnowledge = useCallback(async (id: string) => {
    setKnowledgeBases(prev => prev.map(k => 
      k.id === id ? { ...k, status: 'syncing' as const } : k
    ));

    try {
      await invoke('sync_knowledge_base', {
        workspacePath,
        knowledgeId: id
      });
      
      setKnowledgeBases(prev => prev.map(k => 
        k.id === id ? { ...k, status: 'active' as const, lastSyncAt: new Date().toISOString() } : k
      ));
    } catch (err) {
      log.error('Failed to sync knowledge base', { workspacePath, knowledgeId: id, error: err });
      setKnowledgeBases(prev => prev.map(k => 
        k.id === id ? { ...k, status: 'error' as const, errorMessage: 'Sync failed' } : k
      ));
    }
  }, [workspacePath]);

  const addKnowledge = useCallback(async (data: AddKnowledgeFormData) => {
    try {
      const newKb = await invoke<KnowledgeBaseItem>('add_knowledge_base', {
        workspacePath,
        data
      }).catch(() => {
        // Mock create.
        const id = `${data.type}-${Date.now()}`;
        return {
          id,
          name: data.name,
          description: data.description,
          type: data.type,
          icon: data.icon,
          enabled: true,
          status: 'active' as const,
          tags: data.tags,
          tokenEstimate: data.type === 'skill' ? 1000 : 0,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          config: data.type === 'skill' ? data.skillConfig : data.ragConfig
        } as KnowledgeBaseItem;
      });

      setKnowledgeBases(prev => [...prev, newKb]);
    } catch (err) {
      log.error('Failed to add knowledge base', { workspacePath, error: err });
    }
  }, [workspacePath]);

  const openKnowledge = useCallback((kb: KnowledgeBaseItem) => {
    // TODO: implement details/edit dialog.
  }, []);

  useEffect(() => {
    loadKnowledgeBases();
  }, [loadKnowledgeBases]);

  const { skillBases, ragBases } = useMemo(() => {
    const filtered = knowledgeBases.filter(kb => {
      if (!searchQuery) return true;
      const query = searchQuery.toLowerCase();
      return kb.name.toLowerCase().includes(query) ||
        kb.description.toLowerCase().includes(query) ||
        kb.tags.some(tag => tag.toLowerCase().includes(query));
    });

    return {
      skillBases: filtered.filter(kb => kb.type === 'skill'),
      ragBases: filtered.filter(kb => kb.type === 'rag')
    };
  }, [knowledgeBases, searchQuery]);

  if (loading) {
    return (
      <div className="bitfun-knowledge-tab bitfun-knowledge-tab--loading">
        <RefreshCw size={16} className="bitfun-knowledge-tab__spinner" />
        <span>{t('knowledgeTab.loading')}</span>
      </div>
    );
  }

  return (
    <div className="bitfun-knowledge-tab">
      <div className="bitfun-knowledge-tab__toolbar">
        <Search
          placeholder={t('knowledgeTab.searchPlaceholder')}
          value={searchQuery}
          onChange={setSearchQuery}
          clearable
          size="small"
        />
        <Tooltip content={t('knowledgeTab.addKnowledge')} placement="top">
          <IconButton
            variant="ghost"
            size="xs"
            onClick={() => setShowAddDialog(true)}
          >
            <Plus size={14} />
          </IconButton>
        </Tooltip>
      </div>

      <div className="bitfun-knowledge-tab__content">
        {knowledgeBases.length === 0 ? (
          <div className="bitfun-knowledge-tab__empty">
            <BookOpen size={32} />
            <p>{t('knowledgeTab.empty.title')}</p>
            <span>{t('knowledgeTab.empty.description')}</span>
            <button 
              className="bitfun-knowledge-tab__add-btn"
              onClick={() => setShowAddDialog(true)}
            >
              <Plus size={14} />
              {t('knowledgeTab.addKnowledge')}
            </button>
          </div>
        ) : (
          <>
            <div className="bitfun-knowledge-tab__section">
              <div 
                className="bitfun-knowledge-tab__section-header"
                onClick={() => setSkillExpanded(!skillExpanded)}
              >
                {skillExpanded ? (
                  <ChevronDown size={14} className="bitfun-knowledge-tab__chevron" />
                ) : (
                  <ChevronRight size={14} className="bitfun-knowledge-tab__chevron" />
                )}
                <Brain size={14} className="bitfun-knowledge-tab__section-icon bitfun-knowledge-tab__section-icon--skill" />
                <span className="bitfun-knowledge-tab__section-title">{t('knowledgeTab.sections.skill.title')}</span>
                <span className="bitfun-knowledge-tab__section-count">{skillBases.length}</span>
              </div>

              {skillExpanded && (
                <div className="bitfun-knowledge-tab__section-content">
                  {skillBases.length > 0 ? (
                    <div className="bitfun-knowledge-tab__list">
                      {skillBases.map(kb => (
                        <KnowledgeItem
                          key={kb.id}
                          knowledge={kb}
                          onToggleEnabled={() => toggleKnowledgeEnabled(kb.id)}
                          onOpen={() => openKnowledge(kb)}
                          onDelete={() => setShowDeleteConfirm(kb.id)}
                          onSync={() => syncKnowledge(kb.id)}
                        />
                      ))}
                    </div>
                  ) : (
                    <div className="bitfun-knowledge-tab__section-empty">
                      <span>{t('knowledgeTab.sections.skill.empty')}</span>
                    </div>
                  )}
                </div>
              )}
            </div>

            <div className="bitfun-knowledge-tab__section">
              <div 
                className="bitfun-knowledge-tab__section-header"
                onClick={() => setRagExpanded(!ragExpanded)}
              >
                {ragExpanded ? (
                  <ChevronDown size={14} className="bitfun-knowledge-tab__chevron" />
                ) : (
                  <ChevronRight size={14} className="bitfun-knowledge-tab__chevron" />
                )}
                <Database size={14} className="bitfun-knowledge-tab__section-icon bitfun-knowledge-tab__section-icon--rag" />
                <span className="bitfun-knowledge-tab__section-title">{t('knowledgeTab.sections.rag.title')}</span>
                <span className="bitfun-knowledge-tab__section-count">{ragBases.length}</span>
              </div>

              {ragExpanded && (
                <div className="bitfun-knowledge-tab__section-content">
                  {ragBases.length > 0 ? (
                    <div className="bitfun-knowledge-tab__list">
                      {ragBases.map(kb => (
                        <KnowledgeItem
                          key={kb.id}
                          knowledge={kb}
                          onToggleEnabled={() => toggleKnowledgeEnabled(kb.id)}
                          onOpen={() => openKnowledge(kb)}
                          onDelete={() => setShowDeleteConfirm(kb.id)}
                        />
                      ))}
                    </div>
                  ) : (
                    <div className="bitfun-knowledge-tab__section-empty">
                      <span>{t('knowledgeTab.sections.rag.empty')}</span>
                    </div>
                  )}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      <AddKnowledgeDialog
        isOpen={showAddDialog}
        onClose={() => setShowAddDialog(false)}
        onConfirm={addKnowledge}
      />

      <ConfirmDialog
        isOpen={!!showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(null)}
        onConfirm={() => showDeleteConfirm && deleteKnowledge(showDeleteConfirm)}
        title={t('knowledgeTab.deleteDialog.title', 'Delete knowledge base')}
        message={
          <>
            <p>{t('knowledgeTab.deleteDialog.confirm')}</p>
            <span style={{ color: 'var(--color-text-muted)', fontSize: '12px' }}>{t('knowledgeTab.deleteDialog.hint')}</span>
          </>
        }
        type="warning"
        confirmText={t('knowledgeTab.deleteDialog.delete')}
        cancelText={t('knowledgeTab.deleteDialog.cancel')}
        confirmDanger
      />
    </div>
  );
};

export default KnowledgeTab;

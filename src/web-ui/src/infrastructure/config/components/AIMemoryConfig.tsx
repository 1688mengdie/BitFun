 

import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Brain, Calendar, Tag, ChevronDown, ChevronUp, Plus, Edit2, Trash2, Eye, EyeOff } from 'lucide-react';
import { Search, IconButton, Tooltip, Card, CardBody, FilterPill, FilterPillGroup, Button, Modal, Input, Textarea, Select } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import './AIMemoryConfig.scss';
import { 
  getAllMemories, 
  addMemory, 
  updateMemory, 
  deleteMemory, 
  toggleMemory,
  type AIMemory,
  type MemoryType
} from '../../api/aiMemoryApi';
import { useNotification } from '../../../shared/notification-system';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('AIMemoryConfig');

const AIMemoryConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-memory');
  const notification = useNotification();
  const [searchKeyword, setSearchKeyword] = useState('');
  const [filterType, setFilterType] = useState<MemoryType | 'all'>('all');
  const [expandedMemoryIds, setExpandedMemoryIds] = useState<Set<string>>(new Set());
  const [memories, setMemories] = useState<AIMemory[]>([]);
  const [loading, setLoading] = useState(true);
  const [isDeleting, setIsDeleting] = useState(false);  
  
  
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [editingMemory, setEditingMemory] = useState<AIMemory | null>(null);

  
  useEffect(() => {
    loadMemories();
  }, []);
  
  const loadMemories = async () => {
    try {
      setLoading(true);
      const data = await getAllMemories();
      setMemories(data);
    } catch (error) {
      notification.error(t('messages.loadFailed', { error: String(error) }));
    } finally {
      setLoading(false);
    }
  };
  
  
  
  const memoryTypeMap: Record<MemoryType, { label: string; color: string }> = {
    tech_preference: { label: t('memoryTypes.tech_preference'), color: '#60a5fa' },
    project_context: { label: t('memoryTypes.project_context'), color: '#a78bfa' },
    user_habit: { label: t('memoryTypes.user_habit'), color: '#34d399' },
    code_pattern: { label: t('memoryTypes.code_pattern'), color: '#fbbf24' },
    decision: { label: t('memoryTypes.decision'), color: '#f87171' },
    other: { label: t('memoryTypes.other'), color: '#94a3b8' }
  };

  
  const filteredMemories = memories.filter(memory => {
    
    if (filterType !== 'all' && memory.type !== filterType) {
      return false;
    }

    
    if (searchKeyword) {
      const keyword = searchKeyword.toLowerCase();
      return (
        memory.title.toLowerCase().includes(keyword) ||
        memory.content.toLowerCase().includes(keyword) ||
        memory.tags.some(tag => tag.toLowerCase().includes(keyword)) ||
        memory.source.toLowerCase().includes(keyword)
      );
    }

    return true;
  });

  
  const sortedMemories = [...filteredMemories].sort((a, b) => b.importance - a.importance);

  
  const toggleMemoryExpanded = (memoryId: string) => {
    setExpandedMemoryIds(prev => {
      const next = new Set(prev);
      if (next.has(memoryId)) {
        next.delete(memoryId);
      } else {
        next.add(memoryId);
      }
      return next;
    });
  };
  
  
  const handleDelete = async (id: string, event: React.MouseEvent) => {
    
    event.preventDefault();
    event.stopPropagation();
    
    
    if (isDeleting) {
      return;
    }
    
    
    const confirmed = await window.confirm(t('messages.confirmDelete'));
    
    if (!confirmed) {
      return;
    }
    
    try {
      setIsDeleting(true);  
      await deleteMemory(id);
      notification.success(t('messages.deleteSuccess'));
      await loadMemories();  
    } catch (error) {
      log.error('Failed to delete memory', { memoryId: id, error });
      notification.error(t('messages.deleteFailed', { error: String(error) }));
    } finally {
      setIsDeleting(false);  
    }
  };
  
  
  const handleToggle = async (id: string) => {
    try {
      await toggleMemory(id);
      loadMemories();
    } catch (error) {
      notification.error(t('messages.toggleFailed', { error: String(error) }));
    }
  };
  
  
  const handleAdd = () => {
    setEditingMemory(null);
    setIsAddDialogOpen(true);
  };
  
  
  const handleEdit = (memory: AIMemory) => {
    setEditingMemory(memory);
    setIsAddDialogOpen(true);
  };

  
  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffDays === 0) return t('date.today');
    if (diffDays === 1) return t('date.yesterday');
    if (diffDays < 7) return t('date.daysAgo', { days: diffDays });
    return i18nService.formatDate(date, { year: 'numeric', month: '2-digit', day: '2-digit' });
  };

  
  const renderImportance = (importance: number) => {
    return (
      <div className="bitfun-ai-memory-config__importance">
        {[...Array(5)].map((_, index) => (
          <span
            key={index}
            className={`bitfun-ai-memory-config__importance-dot ${
              index < importance ? 'is-active' : ''
            }`}
          />
        ))}
      </div>
    );
  };

  
  const renderMemoriesList = () => {
    if (loading) {
      return (
        <div className="bitfun-ai-memory-config__empty-state">
          <h3>{t('list.loading')}</h3>
        </div>
      );
    }
    
    if (sortedMemories.length === 0) {
      return (
        <div className="bitfun-ai-memory-config__empty-state">
          <h3>{t('list.empty.title')}</h3>
          <p>{searchKeyword ? t('list.empty.noMatch') : t('list.empty.hint')}</p>
        </div>
      );
    }

    return (
      <div className="bitfun-ai-memory-config__list">
        {sortedMemories.map((memory) => {
          const isExpanded = expandedMemoryIds.has(memory.id);
          const typeInfo = memoryTypeMap[memory.type];

          return (
            <Card
              key={memory.id}
              variant="default"
              padding="none"
              className={`bitfun-ai-memory-config__item ${isExpanded ? 'is-expanded' : ''} ${!memory.enabled ? 'is-disabled' : ''}`}
            >
              
              <div className="bitfun-ai-memory-config__item-header">
                <div 
                  className="bitfun-ai-memory-config__item-main"
                  onClick={() => toggleMemoryExpanded(memory.id)}
                >
                  <div className="bitfun-ai-memory-config__item-title-row">
                    <div className="bitfun-ai-memory-config__item-title">{memory.title}</div>
                    {renderImportance(memory.importance)}
                  </div>
                  <div className="bitfun-ai-memory-config__item-badges">
                    <span
                      className="bitfun-ai-memory-config__badge bitfun-ai-memory-config__badge--type"
                      style={{ backgroundColor: `${typeInfo.color}20`, color: typeInfo.color }}
                    >
                      {typeInfo.label}
                    </span>
                    <span className="bitfun-ai-memory-config__badge bitfun-ai-memory-config__badge--date">
                      <Calendar size={12} />
                      {formatDate(memory.created_at)}
                    </span>
                    {!memory.enabled && (
                      <span className="bitfun-ai-memory-config__badge bitfun-ai-memory-config__badge--disabled">
                        {t('list.item.disabled')}
                      </span>
                    )}
                  </div>
                  {memory.tags.length > 0 && (
                    <div className="bitfun-ai-memory-config__item-tags">
                      {memory.tags.map((tag, index) => (
                        <span key={index} className="bitfun-ai-memory-config__tag">
                          <Tag size={10} />
                          {tag}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                
                
                <div className="bitfun-ai-memory-config__item-actions" onClick={(e) => e.stopPropagation()}>
                  <IconButton 
                    tooltip={memory.enabled ? t('actions.disable') : t('actions.enable')}
                    onClick={() => handleToggle(memory.id)}
                    size="small"
                    variant="ghost"
                  >
                    {memory.enabled ? <Eye size={16} /> : <EyeOff size={16} />}
                  </IconButton>
                  <IconButton 
                    tooltip={t('actions.edit')}
                    onClick={() => handleEdit(memory)}
                    size="small"
                    variant="ghost"
                  >
                    <Edit2 size={16} />
                  </IconButton>
                  <IconButton 
                    tooltip={t('actions.delete')}
                    onClick={(e) => handleDelete(memory.id, e)}
                    size="small"
                    variant="danger"
                  >
                    <Trash2 size={16} />
                  </IconButton>
                  <IconButton 
                    onClick={() => toggleMemoryExpanded(memory.id)}
                    size="small"
                    variant="ghost"
                  >
                    {isExpanded ? <ChevronUp size={20} /> : <ChevronDown size={20} />}
                  </IconButton>
                </div>
              </div>

              
              {isExpanded && (
                <CardBody className="bitfun-ai-memory-config__item-details">
                  <div className="bitfun-ai-memory-config__item-content">
                    <div className="bitfun-ai-memory-config__content-label">{t('list.item.contentLabel')}</div>
                    <div className="bitfun-ai-memory-config__content-text">{memory.content}</div>
                  </div>

                  <div className="bitfun-ai-memory-config__item-meta">
                    <span className="bitfun-ai-memory-config__meta-item">
                      {t('list.item.sourcePrefix')}{memory.source}
                    </span>
                    <span className="bitfun-ai-memory-config__meta-item">
                      {t('list.item.createdPrefix')}{i18nService.formatDate(new Date(memory.created_at))}
                    </span>
                  </div>
                </CardBody>
              )}
            </Card>
          );
        })}
      </div>
    );
  };

  
  const renderMemoryContent = () => {
    const enabledCount = memories.filter(m => m.enabled).length;
    const tagCount = new Set(memories.flatMap(m => m.tags)).size;

    return (
      <>
        
        <div className="bitfun-ai-memory-config__toolbar">
          <div className="bitfun-ai-memory-config__search-box">
            <Search
              placeholder={t('toolbar.searchPlaceholder')}
              value={searchKeyword}
              onChange={(val) => setSearchKeyword(val)}
              clearable
              size="small"
            />
          </div>
          
          <div className="bitfun-ai-memory-config__stats-inline">
            <span className="bitfun-ai-memory-config__stat-text">
              {enabledCount}/{memories.length}
            </span>
            <span className="bitfun-ai-memory-config__stat-separator">·</span>
            <span className="bitfun-ai-memory-config__stat-text">
              {tagCount} {t('stats.tags')}
            </span>
          </div>
          <IconButton 
            variant="primary" 
            size="small"
            onClick={handleAdd} 
            tooltip={t('toolbar.addTooltip')}
          >
            <Plus size={16} />
          </IconButton>
        </div>
        
        <FilterPillGroup className="bitfun-ai-memory-config__filters">
          <FilterPill
            label={t('filters.all')}
            count={memories.length}
            active={filterType === 'all'}
            onClick={() => setFilterType('all')}
          />
          {Object.entries(memoryTypeMap).map(([type, info]) => {
            const count = memories.filter((m: AIMemory) => m.type === type).length;
            return (
              <FilterPill
                key={type}
                label={info.label}
                count={count}
                active={filterType === type}
                onClick={() => setFilterType(type as MemoryType)}
              />
            );
          })}
        </FilterPillGroup>

        
        {renderMemoriesList()}
        
        
        {isAddDialogOpen && (
          <MemoryEditDialog
            memory={editingMemory}
            memoryTypeMap={memoryTypeMap}
            onClose={() => setIsAddDialogOpen(false)}
            onSave={loadMemories}
          />
        )}
      </>
    );
  };

  return (
    <ConfigPageLayout className="bitfun-ai-memory-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="bitfun-ai-memory-config__content">
        <div className="bitfun-ai-memory-config__tab-content">
          {renderMemoryContent()}
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};


interface MemoryEditDialogProps {
  memory: AIMemory | null;
  memoryTypeMap: Record<MemoryType, { label: string; color: string }>;
  onClose: () => void;
  onSave: () => void;
}

const MemoryEditDialog: React.FC<MemoryEditDialogProps> = ({ memory, memoryTypeMap, onClose, onSave }) => {
  const { t } = useTranslation('settings/ai-memory');
  const notification = useNotification();
  const [title, setTitle] = useState(memory?.title || '');
  const [content, setContent] = useState(memory?.content || '');
  const [memoryType, setMemoryType] = useState<MemoryType>(memory?.type || 'other');
  const [importance, setImportance] = useState(memory?.importance || 3);
  const [tags, setTags] = useState(memory?.tags?.join(', ') || '');
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (!title.trim() || !content.trim()) {
      notification.error(t('messages.validationError'));
      return;
    }

    try {
      setSaving(true);
      const tagsArray = tags.split(',').map(t => t.trim()).filter(t => t);
      
      if (memory) {
        
        await updateMemory({
          id: memory.id,
          title,
          content,
          type: memoryType,
          importance,
          tags: tagsArray,
          enabled: memory.enabled
        });
        notification.success(t('messages.updateSuccess'));
      } else {
        
        await addMemory({
          title,
          content,
          type: memoryType,
          importance,
          tags: tagsArray
        });
        notification.success(t('messages.createSuccess'));
      }
      
      onSave();
      onClose();
    } catch (error) {
      notification.error(t('messages.saveFailed', { error: String(error) }));
    } finally {
      setSaving(false);
    }
  };

  const typeOptions = Object.entries(memoryTypeMap).map(([key, info]) => ({
    value: key,
    label: info.label
  }));

  return (
    <Modal
      isOpen={true}
      onClose={onClose}
      title={memory ? t('dialog.titleEdit') : t('dialog.titleCreate')}
      size="medium"
    >
      <div className="bitfun-ai-memory-config__dialog-body">
        <Input
          label={t('dialog.fields.title')}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder={t('dialog.fields.titlePlaceholder')}
        />
        
        <Select
          label={t('dialog.fields.type')}
          options={typeOptions}
          value={memoryType}
          onChange={(val) => setMemoryType(val as MemoryType)}
        />
        
        <div className="bitfun-ai-memory-config__form-group">
          <label>{t('dialog.fields.importance')} ({importance}/5)</label>
          <input
            type="range"
            min="1"
            max="5"
            value={importance}
            onChange={(e) => setImportance(Number(e.target.value))}
          />
        </div>
        
        <Textarea
          label={t('dialog.fields.content')}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder={t('dialog.fields.contentPlaceholder')}
          rows={6}
        />
        
        <Input
          label={t('dialog.fields.tags')}
          value={tags}
          onChange={(e) => setTags(e.target.value)}
          placeholder={t('dialog.fields.tagsPlaceholder')}
        />
      </div>
      
      <div className="bitfun-ai-memory-config__dialog-footer">
        <Button variant="secondary" onClick={onClose} disabled={saving}>{t('dialog.actions.cancel')}</Button>
        <Button variant="primary" onClick={handleSave} disabled={saving} isLoading={saving}>
          {saving ? t('dialog.actions.saving') : t('dialog.actions.save')}
        </Button>
      </div>
    </Modal>
  );
};

export default AIMemoryConfig;


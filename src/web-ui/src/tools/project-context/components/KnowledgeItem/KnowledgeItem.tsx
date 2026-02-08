/**
 * Knowledge base item component with toggle, open, delete, and optional sync.
 */

import React, { useCallback } from 'react';
import {
  Brain,
  Database,
  BookOpen,
  FileText,
  Lightbulb,
  Library,
  Compass,
  Code,
  Terminal,
  Cpu,
  Globe,
  Search,
  Sparkles,
  Zap,
  Trash2,
  ExternalLink,
  RefreshCw,
  AlertCircle,
  Check
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Switch, IconButton, Tooltip } from '@/component-library';
import type { KnowledgeBaseItem, KnowledgeBaseStatus } from '../../types/knowledge';
import './KnowledgeItem.scss';

const ICON_MAP: Record<string, React.ReactNode> = {
  BookOpen: <BookOpen size={16} />,
  Brain: <Brain size={16} />,
  Database: <Database size={16} />,
  FileText: <FileText size={16} />,
  Lightbulb: <Lightbulb size={16} />,
  Library: <Library size={16} />,
  Compass: <Compass size={16} />,
  Code: <Code size={16} />,
  Terminal: <Terminal size={16} />,
  Cpu: <Cpu size={16} />,
  Globe: <Globe size={16} />,
  Search: <Search size={16} />,
  Sparkles: <Sparkles size={16} />,
  Zap: <Zap size={16} />
};

const STATUS_ICONS: Record<KnowledgeBaseStatus, React.ReactNode> = {
  active: <Check size={10} />,
  inactive: null,
  error: <AlertCircle size={10} />,
  syncing: <RefreshCw size={10} className="bitfun-knowledge-item__status-spin" />
};

export interface KnowledgeItemProps {
  /** Knowledge base data. */
  knowledge: KnowledgeBaseItem;
  /** Toggle enabled state. */
  onToggleEnabled: (enabled: boolean) => void;
  /** Open details/edit view. */
  onOpen: () => void;
  /** Delete the knowledge base. */
  onDelete: () => void;
  /** Sync/refresh. */
  onSync?: () => void;
}

export const KnowledgeItem: React.FC<KnowledgeItemProps> = ({
  knowledge,
  onToggleEnabled,
  onOpen,
  onDelete,
  onSync
}) => {
  const { t } = useTranslation('panels/project-context');
  const { name, description, type, icon, enabled, status, tokenEstimate, config } = knowledge;

  const itemIcon = ICON_MAP[icon] || <BookOpen size={16} />;

  const handleClick = useCallback(() => {
    onOpen();
  }, [onOpen]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      onOpen();
    }
  }, [onOpen]);

  const handleToggle = useCallback((checked: boolean) => {
    onToggleEnabled(checked);
  }, [onToggleEnabled]);

  const handleDelete = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete();
  }, [onDelete]);

  const handleSync = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onSync?.();
  }, [onSync]);

  const getSourceInfo = (): string => {
    if (type === 'skill') {
      const skillConfig = config as KnowledgeBaseItem['config'];
      if ('filePath' in skillConfig && skillConfig.filePath) {
        return skillConfig.filePath.split(/[/\\]/).pop() || t('knowledgeTab.item.sourceFile');
      }
      return t('knowledgeTab.item.sourceEmbedded');
    } else {
      const ragConfig = config as KnowledgeBaseItem['config'];
      if ('endpoint' in ragConfig && ragConfig.endpoint) {
        try {
          const url = new URL(ragConfig.endpoint);
          return url.hostname;
        } catch {
          return t('knowledgeTab.item.sourceApi');
        }
      }
      return t('knowledgeTab.item.sourceRag');
    }
  };

  const formatTokens = (count: number): string => {
    if (count >= 1000) {
      return `${(count / 1000).toFixed(1)}k`;
    }
    return count.toString();
  };

  return (
    <div
      className={`bitfun-knowledge-item ${!enabled ? 'bitfun-knowledge-item--disabled' : ''} ${status === 'error' ? 'bitfun-knowledge-item--error' : ''}`}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
    >
      <div className={`bitfun-knowledge-item__icon bitfun-knowledge-item__icon--${type}`}>
        {itemIcon}
        {status !== 'inactive' && STATUS_ICONS[status] && (
          <span className={`bitfun-knowledge-item__status bitfun-knowledge-item__status--${status}`}>
            {STATUS_ICONS[status]}
          </span>
        )}
      </div>

      <div className="bitfun-knowledge-item__info">
        <div className="bitfun-knowledge-item__header">
          <span className="bitfun-knowledge-item__name">{name}</span>
          <span className={`bitfun-knowledge-item__type bitfun-knowledge-item__type--${type}`}>
            {type === 'skill' ? t('knowledgeTab.item.typeSkill') : t('knowledgeTab.item.typeRAG')}
          </span>
        </div>
        <div className="bitfun-knowledge-item__meta">
          <span className="bitfun-knowledge-item__source">{getSourceInfo()}</span>
          <span className="bitfun-knowledge-item__dot">·</span>
          <span className="bitfun-knowledge-item__tokens">~{formatTokens(tokenEstimate)} tokens</span>
        </div>
        {status === 'error' && knowledge.errorMessage && (
          <div className="bitfun-knowledge-item__error">
            <AlertCircle size={10} />
            <span>{knowledge.errorMessage}</span>
          </div>
        )}
      </div>

      <div className="bitfun-knowledge-item__actions">
        {type === 'rag' && 'endpoint' in config && config.endpoint && (
          <Tooltip content={t('knowledgeTab.item.openEndpoint')} placement="top">
            <IconButton
              variant="ghost"
              size="xs"
              onClick={(e) => {
                e.stopPropagation();
                window.open(config.endpoint, '_blank');
              }}
            >
              <ExternalLink size={12} />
            </IconButton>
          </Tooltip>
        )}

        {type === 'skill' && onSync && (
          <Tooltip content={t('knowledgeTab.item.sync')} placement="top">
            <IconButton
              variant="ghost"
              size="xs"
              onClick={handleSync}
              disabled={status === 'syncing'}
            >
              <RefreshCw size={12} className={status === 'syncing' ? 'bitfun-knowledge-item__action-spin' : ''} />
            </IconButton>
          </Tooltip>
        )}

        <Tooltip content={t('knowledgeTab.item.delete')} placement="top">
          <IconButton
            variant="ghost"
            size="xs"
            onClick={handleDelete}
            className="bitfun-knowledge-item__delete-btn"
          >
            <Trash2 size={12} />
          </IconButton>
        </Tooltip>
      </div>

      <div className="bitfun-knowledge-item__switch" onClick={(e) => e.stopPropagation()}>
        <Switch
          size="small"
          checked={enabled}
          onChange={handleToggle}
        />
      </div>
    </div>
  );
};

export default KnowledgeItem;

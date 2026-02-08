import React from 'react';
import './GitStatusIndicator.scss';
import { useI18n } from '@/infrastructure/i18n';

export interface GitStatusIndicatorProps {
  status: 'untracked' | 'modified' | 'added' | 'deleted' | 'renamed' | 'conflicted' | 'staged';
  compact?: boolean;
  className?: string;
}

function getStatusInfo(status: GitStatusIndicatorProps['status'], t: (key: string) => string) {
  switch (status) {
    case 'untracked':
      return {
        label: 'U',
        fullLabel: t('git.status.untracked'),
        color: 'green',
        description: t('git.statusDescription.untracked')
      };
    case 'modified':
      return {
        label: 'M',
        fullLabel: t('git.status.modified'),
        color: 'yellow',
        description: t('git.statusDescription.modified')
      };
    case 'added':
      return {
        label: 'A',
        fullLabel: t('git.status.added'),
        color: 'green',
        description: t('git.statusDescription.added')
      };
    case 'deleted':
      return {
        label: 'D',
        fullLabel: t('git.status.deleted'),
        color: 'red',
        description: t('git.statusDescription.deleted')
      };
    case 'renamed':
      return {
        label: 'R',
        fullLabel: t('git.status.renamed'),
        color: 'purple',
        description: t('git.statusDescription.renamed')
      };
    case 'conflicted':
      return {
        label: 'C',
        fullLabel: t('git.status.conflict'),
        color: 'orange',
        description: t('git.statusDescription.conflicted')
      };
    case 'staged':
      return {
        label: 'M',
        fullLabel: t('git.status.staged'),
        color: 'cyan',
        description: t('git.statusDescription.staged')
      };
    default:
      return {
        label: '?',
        fullLabel: t('git.status.unknown'),
        color: 'gray',
        description: t('git.statusDescription.unknown')
      };
  }
}

export const GitStatusIndicator: React.FC<GitStatusIndicatorProps> = ({
  status,
  compact = false,
  className = ''
}) => {
  const { t } = useI18n('tools');
  const statusInfo = getStatusInfo(status, t);
  
  return (
    <span
      className={`bitfun-git-status-indicator bitfun-git-status-indicator--${statusInfo.color} ${
        compact ? 'bitfun-git-status-indicator--compact' : ''
      } ${className}`}
      title={`${statusInfo.fullLabel}: ${statusInfo.description}`}
      data-status={status}
    >
      {statusInfo.label}
    </span>
  );
};

export default GitStatusIndicator;


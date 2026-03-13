import React from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import WorkspaceItem from './WorkspaceItem';
import './WorkspaceListSection.scss';

interface WorkspaceListSectionProps {
  variant: 'assistants' | 'projects';
}

const WorkspaceListSection: React.FC<WorkspaceListSectionProps> = ({ variant }) => {
  const { t } = useI18n('common');
  const {
    openedWorkspacesList,
    normalWorkspacesList,
    assistantWorkspacesList,
    activeWorkspaceId,
  } = useWorkspaceContext();

  const workspaces = variant === 'assistants'
    ? assistantWorkspacesList
    : normalWorkspacesList;
  const emptyLabel = variant === 'assistants'
    ? t('nav.workspaces.emptyAssistants')
    : t('nav.workspaces.emptyProjects');

  return (
    <div className="bitfun-nav-panel__workspace-list">
      {workspaces.length === 0 ? (
        <div className="bitfun-nav-panel__workspace-list-empty">
          {emptyLabel}
        </div>
      ) : (
        workspaces.map(workspace => (
          <WorkspaceItem
            key={workspace.id}
            workspace={workspace}
            isActive={workspace.id === activeWorkspaceId}
            isSingle={openedWorkspacesList.length === 1}
          />
        ))
      )}
    </div>
  );
};

export default WorkspaceListSection;

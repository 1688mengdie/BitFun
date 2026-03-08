import React, { useEffect } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import WorkspaceItem from './WorkspaceItem';
import './WorkspaceListSection.scss';

const WorkspaceListSection: React.FC = () => {
  const { t } = useI18n('common');
  const { openedWorkspacesList, activeWorkspaceId } = useWorkspaceContext();

  useEffect(() => {
    openedWorkspacesList.forEach(workspace => {
      void flowChatStore.initializeFromDisk(workspace.rootPath);
    });
  }, [openedWorkspacesList]);

  return (
    <div className="bitfun-nav-panel__workspace-list">
      {openedWorkspacesList.length === 0 ? (
        <div className="bitfun-nav-panel__workspace-list-empty">
          {t('nav.workspaces.empty')}
        </div>
      ) : (
        openedWorkspacesList.map(workspace => (
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

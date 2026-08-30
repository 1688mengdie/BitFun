import React, { useCallback, useEffect, useState } from 'react';
import { Users } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { FlowChatState } from '@/flow_chat/types/flow-chat';
import { sessionIsGroupChat } from '@/flow_chat/utils/sessionOrdering';
import SectionHeader from '../../components/SectionHeader';
import SessionsSection from '../sessions/SessionsSection';

interface GroupChatsSectionProps {
  /** Claw default assistant workspace hosting group chat sessions. */
  workspaceId?: string;
  workspacePath?: string;
  /** Remote SSH identity, forwarded to SessionsSection for scoping. */
  remoteConnectionId?: string | null;
  remoteSshHost?: string | null;
  /** R-GC-27: opens the existing CreateGroupChatDialog. */
  onCreateGroupChat: () => void;
}

/**
 * R-WF-12: dedicated "Group Chats" nav section. Self-contained: renders its
 * own section header (collapsible), the single create entry ("New group chat"
 * -> existing CreateGroupChatDialog), and the group-chat-only session list.
 * Reuses SessionsSection with `groupChatsOnly` — no new session machinery.
 */
const GroupChatsSection: React.FC<GroupChatsSectionProps> = ({
  workspaceId,
  workspacePath,
  remoteConnectionId = null,
  remoteSshHost = null,
  onCreateGroupChat,
}) => {
  const { t } = useI18n('common');
  const [isOpen, setIsOpen] = useState(true);
  const [groupChatCount, setGroupChatCount] = useState(0);

  // R-WF-12: track how many group chat sessions exist in this workspace so the
  // section can render its own empty hint ("No group chats yet"). SessionsSection
  // intentionally keeps its empty branch a bare inline-list container (R-NS-01
  // contract), so the empty text lives here instead.
  useEffect(() => {
    const selectCount = (state: FlowChatState) => {
      let count = 0;
      for (const session of state.sessions.values()) {
        if (!sessionIsGroupChat(session)) continue;
        if (!workspacePath || session.workspacePath === workspacePath) {
          count += 1;
        }
      }
      return count;
    };
    setGroupChatCount(selectCount(flowChatStore.getState()));
    return flowChatStore.subscribeSelector(selectCount, setGroupChatCount);
  }, [workspacePath]);

  const toggleOpen = useCallback(() => setIsOpen(open => !open), []);

  const newGroupChatLabel = t('nav.groupChats.newGroupChat');

  return (
    <div
      className="bitfun-nav-panel__section"
      data-bf-component="nav-panel"
      data-bf-part="section"
      data-bf-section="group-chats"
      data-testid="nav-group-chats-section"
    >
      <SectionHeader
        label={t('nav.sections.groupChats')}
        collapsible
        isOpen={isOpen}
        onToggle={toggleOpen}
        actions={
          <div className="bitfun-nav-panel__section-actions" data-bf-component="nav-panel" data-bf-part="groupChatsActions">
            <Tooltip content={newGroupChatLabel} placement="right" followCursor>
              <button
                type="button"
                className="bitfun-nav-panel__section-action"
                aria-label={newGroupChatLabel}
                onClick={onCreateGroupChat}
                data-testid="nav-group-chats-create-group-btn"
              >
                <Users size={13} />
              </button>
            </Tooltip>
          </div>
        }
      />
      <div className={`bitfun-nav-panel__collapsible${isOpen ? '' : ' is-collapsed'}`} data-bf-component="nav-panel" data-bf-part="sectionContent" data-bf-state={isOpen ? 'open' : ''}>
        <div className="bitfun-nav-panel__collapsible-inner">
          <div className="bitfun-nav-panel__items bitfun-nav-panel__items--session-blocks">
            {groupChatCount === 0 ? (
              <div className="bitfun-nav-panel__group-chats-empty" data-bf-component="group-chats" data-bf-part="empty" data-testid="nav-group-chats-empty">
                <span>{t('nav.groupChats.empty')}</span>
              </div>
            ) : null}
            <SessionsSection
              workspaceId={workspaceId}
              workspacePath={workspacePath}
              remoteConnectionId={remoteConnectionId}
              remoteSshHost={remoteSshHost}
              isActiveWorkspace
              assistantLabel={t('nav.sections.groupChats')}
              isVisible={isOpen}
              groupChatsOnly
            />
          </div>
        </div>
      </div>
    </div>
  );
};

export default React.memo(GroupChatsSection);

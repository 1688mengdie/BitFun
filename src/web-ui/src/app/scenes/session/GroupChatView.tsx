/**
 * GroupChatView — group chat session view (R-GC-14 view + R-GC-15 member
 * management & fork).
 *
 * Reuse rules (type-contract section 4, top red line):
 * - Layout = the original session pane (zero hand-rolled bars, R-GC-24):
 *   - Top bar = the existing FlowChatHeader inside ModernFlowChatContainer
 *     (flow_chat/components/modern/ModernFlowChatContainer.tsx:2462-2488).
 *     The group-chat menu (members / invite / fork) is injected into the
 *     existing left action group via `headerLeftActionsContent`
 *     (FlowChatHeader.tsx:490-497), which already hosts SessionFilesBadge.
 *   - Bubble list = existing ModernFlowChatContainer
 *     (flow_chat/components/modern/ModernFlowChatContainer.tsx). History
 *     turns are injected via flowChatStore.addDialogTurn (FlowChatStore.ts:5084)
 *     and rendered by the existing UserMessageItem (senderBadge reads
 *     metadata.senderName/senderSessionId automatically, UserMessageItem.tsx:219).
 *   - Input = existing ChatInput + ChatInputRegistration.onSubmit host contract
 *     (chatInputRegistration.ts:34-60; ChatInput.tsx:5266-5282 explicitly names
 *     the registered-host send button). onSubmit calls
 *     toolAPI.executeTool({ toolName: 'send_group_message', ... })
 *     (ToolAPI.ts:49-61 — the single camelCase execute_tool wrapper).
 * - Member picker (invite/fork) reuses the component-library Select with
 *   multiple + searchable + showSelectAll (Select.tsx:87, exported from
 *   component-library index.ts:21) inside the existing Modal (Modal.tsx:65)
 *   with Button (Button.tsx:15) / Input (Input.tsx:20) actions — no custom
 *   list is built (R-GC-22).
 * - Jump to a forked child group reuses the R-GC-13 handleGroupChatCreated
 *   registration shape: flowChatStore.createSession (FlowChatStore.ts:3744) +
 *   markSessionAsGroupChat (FlowChatStore.ts:7075) + openMainSession
 *   (sessionActivation.ts:7).
 * - Every action goes through execute_tool; bare invoke('*_group_*') is forbidden.
 * - Styles = existing appearance tokens only.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, IconButton, Input, Modal } from '@/component-library';
import { UserPlus, GitBranch, Users } from 'lucide-react';
import { ModernFlowChatContainer as FlowChatContainer } from '../../../flow_chat/components/modern/ModernFlowChatContainer';
import { ChatInput } from '../../../flow_chat/components/ChatInput';
import type { ChatInputRegistration, ChatInputSubmission } from '../../../flow_chat/components/chatInputRegistration';
import { flowChatStore } from '../../../flow_chat/store/FlowChatStore';
import { openMainSession } from '../../../flow_chat/services/sessionActivation';
import type { DialogTurn } from '../../../flow_chat/types/flow-chat';
import type { DialogTurnKind } from '@/shared/types/session-history';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionMetadata } from '@/shared/types/session-history';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('GroupChatView');

const HISTORY_LIMIT = 200;

// R-GC-28: max members created per invite/fork (count-driven; members are
// always fresh unique-UUID sessions, never reused existing ids).
const MAX_MEMBER_COUNT = 20;

interface GroupChatViewProps {
  /** Group session id (== session id). */
  groupId: string;
  /** Group session workspace rootPath. */
  workspacePath: string;
  /** Current session name (header display). */
  groupName?: string;
  /** Whether the view is the active scene (passed to FlowChatContainer for virtualization/scroll). */
  isSceneActive?: boolean;
}

/**
 * Group message history -> DialogTurn (reuses the existing DialogTurn shape;
 * metadata carries the five sender fields). Mirrors the backend
 * group_room_tools.rs GroupMessage wire 1:1 (author.sessionId/role/depth/name).
 */
function groupMessageToDialogTurn(
  message: {
    messageId?: string;
    content: string;
    timestamp?: number;
    author?: { sessionId?: string; role?: string | null; depth?: number | null; name?: string | null };
    groupSessionId?: string;
  },
  groupId: string,
): DialogTurn {
  const author = message.author ?? {};
  const now = Date.now();
  const timestamp = typeof message.timestamp === 'number' && message.timestamp > 0
    ? message.timestamp
    : now;
  const id = message.messageId || `${groupId}-msg-${timestamp}-${Math.random().toString(36).slice(2, 8)}`;
  const kind: DialogTurnKind = 'user_dialog';
  return {
    id,
    sessionId: groupId,
    kind,
    agentType: 'Claw',
    userMessage: {
      id,
      content: message.content,
      timestamp,
      metadata: {
        groupId,
        senderSessionId: author.sessionId || 'unknown',
        ...(author.role ? { senderRole: author.role } : {}),
        ...(typeof author.depth === 'number' ? { senderDepth: author.depth } : {}),
        ...(author.name ? { senderName: author.name } : {}),
      },
    },
    modelRounds: [],
    status: 'completed',
    startTime: timestamp,
    endTime: timestamp,
    success: true,
    finishReason: 'completed',
    hasFinalResponse: false,
  };
}

/**
 * Resolve the last persisted turn id of a group session. fork_group_chat
 * requires a source_turn_id that matches a persisted turn (branch_session
 * errors with NotFound otherwise), and the backend send flow uses the same id
 * as messageId and turn_id. Falls back to the newest locally injected turn.
 */
function lastTurnIdOf(session: { dialogTurns?: DialogTurn[] } | undefined): string | undefined {
  const turns = session?.dialogTurns;
  if (!turns || turns.length === 0) return undefined;
  const last = turns[turns.length - 1];
  return last?.id || last?.userMessage?.id || undefined;
}

export const GroupChatView: React.FC<GroupChatViewProps> = ({
  groupId,
  workspacePath,
  groupName,
  isSceneActive = true,
}) => {
  const { t } = useI18n('common');
  const [isLoadingHistory, setIsLoadingHistory] = useState(false);
  const [historyFailed, setHistoryFailed] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [, forceRender] = useState(0);

  // R-GC-15: member management state (dialogs only; no custom bars, R-GC-24).
  const [isMembersOpen, setIsMembersOpen] = useState(false);
  const [memberIds, setMemberIds] = useState<string[]>([]);
  const [memberMetaById, setMemberMetaById] = useState<Map<string, SessionMetadata>>(new Map());
  const [isLoadingMembers, setIsLoadingMembers] = useState(false);
  const [membersLoadFailed, setMembersLoadFailed] = useState(false);
  const [isInviteOpen, setIsInviteOpen] = useState(false);
  const [isForkOpen, setIsForkOpen] = useState(false);
  const [isMutatingMember, setIsMutatingMember] = useState(false);
  const membersInitRef = React.useRef(false);

  const lastTurnId = useMemo(
    () => lastTurnIdOf(flowChatStore.getState().sessions.get(groupId)),
    // re-read on render; flowChatStore updates are surfaced via forceRender.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [groupId, isSending],
  );

  const loadHistory = useCallback(async () => {
    if (!groupId) return;
    setIsLoadingHistory(true);
    setHistoryFailed(false);
    try {
      const response = await toolAPI.executeTool({
        toolName: 'get_group_history',
        parameters: { action: 'history', group_id: groupId, limit: HISTORY_LIMIT },
        workspacePath,
      });
      const messages = response?.result?.messages;
      if (response?.success === true && Array.isArray(messages)) {
        // Inject in chronological order (backend returns time-ordered); turns
        // already in the local store are skipped (addDialogTurn dedups by id).
        for (const message of messages as Array<Parameters<typeof groupMessageToDialogTurn>[0]>) {
          if (!message || typeof message.content !== 'string') continue;
          flowChatStore.addDialogTurn(groupId, groupMessageToDialogTurn(message, groupId));
        }
      } else {
        log.warn('get_group_history returned an unexpected response', {
          success: response?.success,
          error: response?.error || response?.validation_error,
        });
        setHistoryFailed(true);
      }
    } catch (error) {
      log.warn('Failed to load group history', { groupId, error });
      setHistoryFailed(true);
    } finally {
      setIsLoadingHistory(false);
      forceRender(v => v + 1);
    }
  }, [groupId, workspacePath]);

  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  // R-GC-15: member list = group session customMetadata.groupChats (member
  // session ids) resolved against the existing session list (display names).
  // Reuses sessionAPI.loadSessionMetadata + sessionAPI.listSessions (the same
  // data source the R-GC-13 member picker uses); no new storage is built.
  const loadMembers = useCallback(async () => {
    if (!groupId || !workspacePath) return;
    setIsLoadingMembers(true);
    setMembersLoadFailed(false);
    try {
      const metadata = await sessionAPI.loadSessionMetadata(groupId, workspacePath);
      const raw = metadata?.customMetadata?.groupChats;
      const ids: string[] = Array.isArray(raw)
        ? raw.filter((v): v is string => typeof v === 'string')
        : [];
      setMemberIds(ids);

      // Resolve display names from the same listSessions source used by the
      // R-GC-13 picker (contract section 1.2). Missing sessions fall back to
      // their raw session id.
      const listResponse = await sessionAPI.listSessions(workspacePath);
      const list = Array.isArray(listResponse) ? listResponse : [];
      const byId = new Map(list.map(meta => [meta.sessionId, meta] as const));
      setMemberMetaById(new Map(ids.map(id => [id, byId.get(id)]).filter(
        (entry): entry is [string, SessionMetadata] => entry[1] !== undefined,
      )));
    } catch (error) {
      log.warn('Failed to load group members', { groupId, error });
      setMembersLoadFailed(true);
    } finally {
      setIsLoadingMembers(false);
    }
  }, [groupId, workspacePath]);

  useEffect(() => {
    if (membersInitRef.current) return;
    membersInitRef.current = true;
    void loadMembers();
  }, [loadMembers]);

  // R-GC-15: invite — invite_group_member (contract section 1.4, camelCase
  // execute_tool wrapper). workspace passed to the backend = current
  // workspacePath (contract section 2a / group_room_tools.rs invite path).
  const handleInvite = useCallback(async (selectedIds: string[]) => {
    if (selectedIds.length === 0 || isMutatingMember) return;
    setIsMutatingMember(true);
    try {
      let successCount = 0;
      for (const memberSessionId of selectedIds) {
        const response = await toolAPI.executeTool({
          toolName: 'invite_group_member',
          parameters: {
            action: 'invite',
            group_id: groupId,
            member_session_id: memberSessionId,
            workspace: workspacePath,
          },
          workspacePath,
        });
        if (response?.success !== true) {
          const message =
            response?.error ||
            response?.validation_error ||
            t('nav.groupChats.inviteFailed');
          notificationService.error(message, { duration: 4000 });
          continue;
        }
        successCount += 1;
      }
      if (successCount > 0) {
        notificationService.success(
          t('nav.groupChats.invited', { count: successCount }),
          { duration: 3000 },
        );
        await loadMembers();
      }
    } catch (error) {
      log.error('Failed to invite group members', { groupId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.inviteFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsMutatingMember(false);
    }
  }, [groupId, isMutatingMember, loadMembers, t, workspacePath]);

  // R-GC-15: remove — remove_group_member.
  const handleRemove = useCallback(async (memberSessionId: string) => {
    if (isMutatingMember) return;
    setIsMutatingMember(true);
    try {
      const response = await toolAPI.executeTool({
        toolName: 'remove_group_member',
        parameters: { action: 'remove', group_id: groupId, member_session_id: memberSessionId },
        workspacePath,
      });
      if (response?.success !== true) {
        const message =
          response?.error ||
          response?.validation_error ||
          t('nav.groupChats.removeFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      notificationService.success(t('nav.groupChats.removed'), { duration: 3000 });
      await loadMembers();
    } catch (error) {
      log.error('Failed to remove group member', { groupId, memberSessionId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.removeFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsMutatingMember(false);
    }
  }, [groupId, isMutatingMember, loadMembers, t, workspacePath]);

  // R-GC-15: fork — fork_group_chat then jump to the child group view.
  // Reuses the R-GC-13 handleGroupChatCreated registration shape
  // (createSession + markSessionAsGroupChat + openMainSession).
  const handleFork = useCallback(async (name: string, memberIds: string[]) => {
    if (isMutatingMember) return;
    const turnId = lastTurnId;
    if (!turnId) {
      notificationService.error(t('nav.groupChats.forkNeedsMessage'), { duration: 4000 });
      return;
    }
    setIsMutatingMember(true);
    try {
      const response = await toolAPI.executeTool({
        toolName: 'fork_group_chat',
        parameters: {
          action: 'fork',
          group_id: groupId,
          name,
          turn_id: turnId,
          members: memberIds,
        },
        workspacePath,
      });
      const childGroupId = response?.result?.childGroupId;
      if (response?.success !== true || typeof childGroupId !== 'string' || !childGroupId) {
        const message =
          response?.error ||
          response?.validation_error ||
          t('nav.groupChats.forkFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      notificationService.success(t('nav.groupChats.forked', { name }), { duration: 3000 });
      // Jump to the child group view (R-GC-15 acceptance: fork -> child view).
      flowChatStore.createSession(
        childGroupId,
        {
          workspacePath,
          projectWorkspacePath: workspacePath,
          agentType: 'Claw',
        },
        undefined,
        name,
        1048576,
        'Claw',
        workspacePath,
      );
      flowChatStore.markSessionAsGroupChat(childGroupId);
      await openMainSession(childGroupId, {});
    } catch (error) {
      log.error('Failed to fork group chat', { groupId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.forkFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsMutatingMember(false);
    }
  }, [groupId, isMutatingMember, lastTurnId, t, workspacePath]);

  const handleSubmit = useCallback(async (submission: ChatInputSubmission) => {
    const content = submission.text?.trim();
    if (!content || isSending || !groupId) return;
    setIsSending(true);
    try {
      // Contract section 1.4: go through execute_tool (camelCase). Bare
      // invoke('send_group_message') is forbidden (R-GC-05 removed the command).
      const response = await toolAPI.executeTool({
        toolName: 'send_group_message',
        parameters: {
          action: 'send',
          group_id: groupId,
          content,
          sender_session_id: groupId,
        },
        workspacePath,
      });
      if (response?.success !== true) {
        const message = response?.error || response?.validation_error || t('nav.groupChats.sendFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      // R-GC-26: the backend routes the message into the group session's real
      // dialog turn (coordinator.start_dialog_turn), which emits
      // DialogTurnStarted + streaming events. The event handler creates the
      // turn and renders the group master response; no local optimistic
      // injection is needed (a local turn would duplicate the backend turn).
    } catch (error) {
      log.error('Failed to send group message', { groupId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.sendFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsSending(false);
    }
  }, [groupId, isSending, t, workspacePath]);

  const registration = useMemo<ChatInputRegistration>(
    () => ({
      registrationId: `group-chat:${groupId}`,
      placeholder: t('nav.groupChats.messagePlaceholder'),
      workspacePath,
      onSubmit: handleSubmit,
    }),
    [groupId, handleSubmit, t, workspacePath],
  );

  // R-GC-15: member rows — name from listSessions metadata, fallback raw id.
  const memberRows = useMemo(
    () => memberIds.map(id => ({ id, name: memberMetaById.get(id)?.sessionName || id })),
    [memberIds, memberMetaById],
  );

  // R-GC-24: group chat menu rendered inside the original FlowChatHeader left
  // action group (reuses IconButton + Modal + Select; no custom top bar).
  const headerLeftActionsContent = useMemo(() => (
    <div
      className="group-chat-view__header-actions"
      data-bf-component="group-chat-view"
      data-bf-part="headerActions"
    >
      <IconButton
        variant="ghost"
        size="xs"
        aria-label={t('nav.groupChats.membersLabel', { count: memberRows.length })}
        tooltip={t('nav.groupChats.membersLabel', { count: memberRows.length })}
        data-testid="group-chat-members-toggle"
        onClick={() => setIsMembersOpen(true)}
      >
        <Users size={14} aria-hidden="true" />
      </IconButton>
      <IconButton
        variant="ghost"
        size="xs"
        aria-label={t('nav.groupChats.invite')}
        tooltip={t('nav.groupChats.invite')}
        onClick={() => setIsInviteOpen(true)}
      >
        <UserPlus size={14} aria-hidden="true" />
      </IconButton>
      <IconButton
        variant="ghost"
        size="xs"
        aria-label={t('nav.groupChats.fork')}
        tooltip={t('nav.groupChats.fork')}
        onClick={() => setIsForkOpen(true)}
      >
        <GitBranch size={14} aria-hidden="true" />
      </IconButton>
    </div>
  ), [memberRows.length, t]);

  const emptyState = useMemo(
    () => (
      <div className="group-chat-view__empty" data-bf-component="group-chat-view" data-bf-part="emptyState">
        {t('nav.groupChats.viewHint')}
      </div>
    ),
    [t],
  );

  return (
    <div
      className="group-chat-view"
      data-bf-component="group-chat-view"
      data-bf-part="root"
      data-testid="group-chat-view"
      data-group-id={groupId}
    >
      <div className="group-chat-view__body" data-bf-component="group-chat-view" data-bf-part="body">
        {isLoadingHistory && !flowChatStore.getState().sessions.get(groupId)?.dialogTurns.length ? (
          <div className="group-chat-view__state">{t('nav.sessions.loading')}</div>
        ) : historyFailed && !flowChatStore.getState().sessions.get(groupId)?.dialogTurns.length ? (
          <div className="group-chat-view__state">
            {t('nav.groupChats.historyLoadFailed')}
            <button
              type="button"
              className="group-chat-view__retry"
              onClick={() => { void loadHistory(); }}
            >
              {t('actions.retry')}
            </button>
          </div>
        ) : (
          <FlowChatContainer
            className="group-chat-view__chat-container"
            isViewportActive={isSceneActive}
            emptyState={emptyState}
            headerLeftActionsContent={headerLeftActionsContent}
            onOpenVisualization={() => {}}
            onFileViewRequest={() => {}}
            onTabOpen={() => {}}
            onSwitchToChatPanel={() => {}}
            config={{ enableMarkdown: true, autoScroll: true, showTimestamps: false }}
          />
        )}
      </div>

      <div className="group-chat-view__input" data-bf-component="group-chat-view" data-bf-part="input">
        <ChatInput
          isSceneActive={isSceneActive}
          onSendMessage={(_message: string) => {}}
          registration={registration}
        />
      </div>

      {isMembersOpen ? (
        <GroupMembersDialog
          groupName={groupName}
          memberRows={memberRows}
          isLoading={isLoadingMembers}
          loadFailed={membersLoadFailed}
          busy={isMutatingMember}
          onRetry={() => { void loadMembers(); }}
          onClose={() => setIsMembersOpen(false)}
          onRemove={handleRemove}
        />
      ) : null}

      {isInviteOpen ? (
        <GroupMemberPickerDialog
          title={t('nav.groupChats.inviteTitle')}
          isOpen={isInviteOpen}
          busy={isMutatingMember}
          onClose={() => setIsInviteOpen(false)}
          onConfirm={handleInvite}
        />
      ) : null}

      {isForkOpen ? (
        <GroupForkDialog
          groupName={groupName}
          isOpen={isForkOpen}
          busy={isMutatingMember}
          onClose={() => setIsForkOpen(false)}
          onConfirm={handleFork}
        />
      ) : null}
    </div>
  );
};

/**
 * R-GC-24: member list dialog. Reuses Modal + Button (component-library);
 * rows render the existing member list shape. No custom top bar.
 */
interface GroupMembersDialogProps {
  groupName?: string;
  memberRows: Array<{ id: string; name: string }>;
  isLoading: boolean;
  loadFailed: boolean;
  busy: boolean;
  onRetry: () => void;
  onClose: () => void;
  onRemove: (memberSessionId: string) => void | Promise<void>;
}

function GroupMembersDialog({
  groupName,
  memberRows,
  isLoading,
  loadFailed,
  busy,
  onRetry,
  onClose,
  onRemove,
}: GroupMembersDialogProps) {
  const { t } = useI18n('common');
  return (
    <Modal
      isOpen
      onClose={busy ? () => {} : onClose}
      title={groupName || t('nav.groupChats.untitled')}
      size="small"
      closeOnOverlayClick={!busy}
    >
      <div data-bf-component="group-member-list-dialog" data-bf-part="root" className="group-chat-dialog">
        {isLoading ? (
          <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
        ) : loadFailed ? (
          <div className="group-chat-dialog__state">
            {t('nav.groupChats.membersLoadFailed')}
            <Button type="button" variant="secondary" size="small" onClick={onRetry}>
              {t('actions.retry')}
            </Button>
          </div>
        ) : memberRows.length === 0 ? (
          <div className="group-chat-dialog__state">{t('nav.groupChats.noMembers')}</div>
        ) : (
          <div className="group-chat-dialog__member-list" data-testid="group-chat-member-list">
            {memberRows.map(member => (
              <div
                key={member.id}
                className="group-chat-dialog__member-row"
                data-bf-component="group-member-list-dialog"
                data-bf-part="memberRow"
                data-member-id={member.id}
              >
                <span className="group-chat-dialog__member-name">{member.name}</span>
                <Button
                  type="button"
                  variant="ghost"
                  size="small"
                  disabled={busy}
                  onClick={() => { void onRemove(member.id); }}
                >
                  {t('nav.groupChats.remove')}
                </Button>
              </div>
            ))}
          </div>
        )}

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.close')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/**
 * R-GC-22/28: member picker dialog (invite). R-GC-28: the invite dialog lets
 * the user enter the number of members to CREATE (fresh unique-UUID Claw
 * sessions); it never lists existing sessions (list_sessions forbidden as a
 * member source). Reuses the component-library Input inside the existing Modal.
 */
interface GroupMemberPickerDialogProps {
  title: string;
  isOpen: boolean;
  busy: boolean;
  onClose: () => void;
  onConfirm: (selectedIds: string[]) => void | Promise<void>;
}

function GroupMemberPickerDialog({
  title,
  isOpen,
  busy,
  onClose,
  onConfirm,
}: GroupMemberPickerDialogProps) {
  const { t } = useI18n('common');
  const [memberCount, setMemberCount] = useState(0);

  useEffect(() => {
    if (!isOpen) {
      setMemberCount(0);
      return;
    }
  }, [isOpen]);

  const parsedMemberCount = Number.isFinite(memberCount)
    ? Math.max(0, Math.min(MAX_MEMBER_COUNT, Math.floor(memberCount)))
    : 0;

  return (
    <Modal
      isOpen={isOpen}
      onClose={busy ? () => {} : onClose}
      title={title}
      size="medium"
      closeOnOverlayClick={!busy}
    >
      <div data-bf-component="group-member-picker-dialog" data-bf-part="root" className="group-chat-dialog">
        <div className="group-chat-dialog__field">
          {/* R-GC-28: invite CREATES fresh member sessions (unique UUID + Claw
              type + Claw name); the picker never lists existing sessions. */}
          <Input
            label={t('nav.groupChats.inviteCount')}
            type="number"
            min={0}
            max={MAX_MEMBER_COUNT}
            value={memberCount}
            onChange={e => setMemberCount(Number(e.target.value))}
            inputSize="medium"
          />
        </div>

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => {
              // R-GC-28: placeholders only; backend creates N fresh members.
              const ids = Array.from({ length: parsedMemberCount }, (_, i) => `member-${i + 1}`);
              void onConfirm(ids);
            }}
            disabled={busy || parsedMemberCount === 0}
            isLoading={busy}
          >
            {t('nav.groupChats.confirmInvite')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/**
 * R-GC-15/28: fork dialog — child group name + member count. R-GC-28: fork
 * members are CREATED fresh (unique-UUID Claw sessions), never selected from
 * existing sessions. Reuses the component-library Input inside the existing
 * Modal; no custom picker is built.
 */
interface GroupForkDialogProps {
  groupName?: string;
  isOpen: boolean;
  busy: boolean;
  onClose: () => void;
  onConfirm: (name: string, memberIds: string[]) => void | Promise<void>;
}

function GroupForkDialog({
  groupName,
  isOpen,
  busy,
  onClose,
  onConfirm,
}: GroupForkDialogProps) {
  const { t } = useI18n('common');
  const [name, setName] = useState('');
  const [memberCount, setMemberCount] = useState(0);

  // Only seed the default child-group name when the dialog opens; never reset
  // the user's typed name on unrelated re-renders (t/groupName excluded from
  // deps for that reason).
  const forkOpenedRef = React.useRef(false);
  useEffect(() => {
    if (!isOpen) {
      forkOpenedRef.current = false;
      setName('');
      setMemberCount(0);
      return;
    }
    if (!forkOpenedRef.current) {
      forkOpenedRef.current = true;
      setName(`${groupName || t('nav.groupChats.untitled')} ${t('nav.groupChats.forkSuffix')}`);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  const parsedMemberCount = Number.isFinite(memberCount)
    ? Math.max(0, Math.min(MAX_MEMBER_COUNT, Math.floor(memberCount)))
    : 0;

  const trimmedName = name.trim();

  return (
    <Modal
      isOpen={isOpen}
      onClose={busy ? () => {} : onClose}
      title={t('nav.groupChats.forkTitle')}
      size="medium"
      closeOnOverlayClick={!busy}
    >
      <div data-bf-component="group-fork-dialog" data-bf-part="root" className="group-chat-dialog">
        <div className="group-chat-dialog__field">
          <Input
            label={t('nav.groupChats.groupName')}
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder={t('nav.groupChats.groupNamePlaceholder')}
            inputSize="medium"
            autoFocus
          />
        </div>

        <div className="group-chat-dialog__field">
          {/* R-GC-28: fork members are CREATED fresh (unique UUID + Claw type
              + Claw name); the picker never lists existing sessions. */}
          <Input
            label={t('nav.groupChats.forkMemberCount')}
            type="number"
            min={0}
            max={MAX_MEMBER_COUNT}
            value={memberCount}
            onChange={e => setMemberCount(Number(e.target.value))}
            inputSize="medium"
          />
        </div>

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => {
              // R-GC-28: placeholders only; backend creates N fresh members.
              const ids = Array.from({ length: parsedMemberCount }, (_, i) => `member-${i + 1}`);
              void onConfirm(trimmedName, ids);
            }}
            disabled={busy || !trimmedName}
            isLoading={busy}
          >
            {t('nav.groupChats.confirmFork')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

export default GroupChatView;

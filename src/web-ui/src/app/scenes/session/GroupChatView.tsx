/**
 * GroupChatView — group chat session view (R-GC-14 view + R-GC-15 member
 * management & fork).
 *
 * Reuse rules (type-contract section 4, top red line):
 * - Bubble list = existing ModernFlowChatContainer
 *   (flow_chat/components/modern/ModernFlowChatContainer.tsx:123). History
 *   turns are injected via flowChatStore.addDialogTurn (FlowChatStore.ts:5071)
 *   and rendered by the existing UserMessageItem (senderBadge reads
 *   metadata.senderName/senderSessionId automatically, UserMessageItem.tsx:202).
 * - Input = existing ChatInput + ChatInputRegistration.onSubmit host contract
 *   (chatInputRegistration.ts:34-60; ChatInput.tsx:4178-4201 explicitly names
 *   the "group chat pane" scenario). onSubmit calls
 *   toolAPI.executeTool({ toolName: 'send_group_message', ... })
 *   (ToolAPI.ts:49-61 — the single camelCase execute_tool wrapper).
 * - Member list / invite / fork dialogs reuse existing components only:
 *   Modal (Modal.tsx:65), Button (Button.tsx:15), Checkbox (Checkbox.tsx:19),
 *   Input (Input.tsx:20) and the session-list + Checkbox multi-select shape of
 *   WorkspaceSessionBatchModal.tsx:305-459 / CreateGroupChatDialog.tsx:177-196.
 * - Jump to a forked child group reuses the R-GC-13 handleGroupChatCreated
 *   registration shape: flowChatStore.createSession (FlowChatStore.ts:3744) +
 *   markSessionAsGroupChat (FlowChatStore.ts:7075) + openMainSession
 *   (sessionActivation.ts:7).
 * - Every action goes through execute_tool; bare invoke('*_group_*') is forbidden.
 * - Styles = existing appearance tokens only.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, Checkbox, Input, Modal } from '@/component-library';
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
import './GroupChatView.scss';

const log = createLogger('GroupChatView');

const HISTORY_LIMIT = 200;

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

  // R-GC-15: member management state.
  const [isMembersOpen, setIsMembersOpen] = useState(true);
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
    if (isMembersOpen) {
      void loadMembers();
    }
  }, [isMembersOpen, loadMembers]);

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
      // Local optimistic injection: the bubble + senderBadge appear
      // immediately (startAutoSync mirrors the active session to the modern store).
      const messageId = response?.result?.messageId;
      const now = Date.now();
      const turn = groupMessageToDialogTurn(
        {
          messageId: typeof messageId === 'string' && messageId ? messageId : undefined,
          content,
          timestamp: now,
          author: { sessionId: groupId, name: groupName || null },
          groupSessionId: groupId,
        },
        groupId,
      );
      flowChatStore.addDialogTurn(groupId, turn);
    } catch (error) {
      log.error('Failed to send group message', { groupId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.sendFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsSending(false);
    }
  }, [groupId, groupName, isSending, t, workspacePath]);

  const registration = useMemo<ChatInputRegistration>(
    () => ({
      registrationId: `group-chat:${groupId}`,
      placeholder: t('nav.groupChats.messagePlaceholder'),
      workspacePath,
      onSubmit: handleSubmit,
    }),
    [groupId, handleSubmit, t, workspacePath],
  );

  const lastSession = flowChatStore.getState().sessions.get(groupId);
  const emptyState = useMemo(
    () => (
      <div className="group-chat-view__empty" data-bf-component="group-chat-view" data-bf-part="emptyState">
        {t('nav.groupChats.viewHint')}
      </div>
    ),
    [t],
  );

  // R-GC-15: member rows — name from listSessions metadata, fallback raw id.
  const memberRows = useMemo(
    () => memberIds.map(id => ({ id, name: memberMetaById.get(id)?.sessionName || id })),
    [memberIds, memberMetaById],
  );

  return (
    <div
      className="group-chat-view"
      data-bf-component="group-chat-view"
      data-bf-part="root"
      data-testid="group-chat-view"
      data-group-id={groupId}
    >
      <div className="group-chat-view__header" data-bf-component="group-chat-view" data-bf-part="header">
        <span className="group-chat-view__title">{groupName || t('nav.groupChats.untitled')}</span>
        <span className="group-chat-view__member-badge" data-bf-component="group-chat-view" data-bf-part="memberBadge">
          {t('nav.groupChats.group')}
        </span>
        <button
          type="button"
          className="group-chat-view__members-toggle"
          data-bf-component="group-chat-view"
          data-bf-part="membersToggle"
          aria-expanded={isMembersOpen}
          onClick={() => setIsMembersOpen(v => !v)}
        >
          {isMembersOpen ? t('nav.groupChats.hideMembers') : t('nav.groupChats.showMembers')}
        </button>
      </div>

      {isMembersOpen ? (
        <div
          className="group-chat-view__members"
          data-bf-component="group-chat-view"
          data-bf-part="members"
          data-testid="group-chat-members"
        >
          <div className="group-chat-view__members-toolbar">
            <span className="group-chat-view__members-label">
              {t('nav.groupChats.membersLabel', { count: memberRows.length })}
            </span>
            <div className="group-chat-view__members-actions">
              <Button type="button" variant="secondary" size="small" onClick={() => setIsInviteOpen(true)}>
                {t('nav.groupChats.invite')}
              </Button>
              <Button type="button" variant="ghost" size="small" onClick={() => setIsForkOpen(true)}>
                {t('nav.groupChats.fork')}
              </Button>
            </div>
          </div>

          {isLoadingMembers ? (
            <div className="group-chat-view__state">{t('nav.sessions.loading')}</div>
          ) : membersLoadFailed ? (
            <div className="group-chat-view__state">
              {t('nav.groupChats.membersLoadFailed')}
              <button
                type="button"
                className="group-chat-view__retry"
                onClick={() => { void loadMembers(); }}
              >
                {t('actions.retry')}
              </button>
            </div>
          ) : memberRows.length === 0 ? (
            <div className="group-chat-view__state">{t('nav.groupChats.noMembers')}</div>
          ) : (
            <div className="group-chat-view__member-list" data-testid="group-chat-member-list">
              {memberRows.map(member => (
                <div
                  key={member.id}
                  className="group-chat-view__member-row"
                  data-bf-component="group-chat-view"
                  data-bf-part="memberRow"
                  data-member-id={member.id}
                >
                  <span className="group-chat-view__member-name">{member.name}</span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="small"
                    disabled={isMutatingMember}
                    onClick={() => { void handleRemove(member.id); }}
                  >
                    {t('nav.groupChats.remove')}
                  </Button>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : null}

      <div className="group-chat-view__body" data-bf-component="group-chat-view" data-bf-part="body">
        {isLoadingHistory && !lastSession?.dialogTurns.length ? (
          <div className="group-chat-view__state">{t('nav.sessions.loading')}</div>
        ) : historyFailed && !lastSession?.dialogTurns.length ? (
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

      {isInviteOpen ? (
        <GroupMemberPickerDialog
          title={t('nav.groupChats.inviteTitle')}
          workspacePath={workspacePath}
          isOpen={isInviteOpen}
          busy={isMutatingMember}
          onClose={() => setIsInviteOpen(false)}
          onConfirm={handleInvite}
        />
      ) : null}

      {isForkOpen ? (
        <GroupForkDialog
          groupName={groupName}
          workspacePath={workspacePath}
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
 * R-GC-15: member picker dialog (invite). Reuses the existing session-list +
 * Checkbox multi-select shape (WorkspaceSessionBatchModal.tsx:305-459 and
 * CreateGroupChatDialog.tsx:177-196) — the same member picker shape the
 * create-group flow already uses; no new picker is built.
 */
interface GroupMemberPickerDialogProps {
  title: string;
  workspacePath: string;
  isOpen: boolean;
  busy: boolean;
  onClose: () => void;
  onConfirm: (selectedIds: string[]) => void | Promise<void>;
}

function GroupMemberPickerDialog({
  title,
  workspacePath,
  isOpen,
  busy,
  onClose,
  onConfirm,
}: GroupMemberPickerDialogProps) {
  const { t } = useI18n('common');
  const [sessions, setSessions] = useState<SessionMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const loadSessions = useCallback(() => {
    setIsLoading(true);
    setLoadFailed(false);
    return sessionAPI
      .listSessions(workspacePath)
      .then(list => setSessions(list.filter(meta => meta.agentType === 'Claw')))
      .catch(error => {
        log.warn('Failed to load Claw sessions for member picker', { error, workspacePath });
        setLoadFailed(true);
      })
      .finally(() => setIsLoading(false));
  }, [workspacePath]);

  useEffect(() => {
    if (!isOpen) {
      setSelectedIds(new Set());
      setLoadFailed(false);
      return;
    }
    void loadSessions();
  }, [isOpen, loadSessions]);

  const toggle = useCallback((sessionId: string) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(sessionId)) {
        next.delete(sessionId);
      } else {
        next.add(sessionId);
      }
      return next;
    });
  }, []);

  const allIds = sessions.map(meta => meta.sessionId);
  const allSelected = allIds.length > 0 && selectedIds.size === allIds.length;

  const toggleSelectAll = useCallback(() => {
    setSelectedIds(prev => (prev.size === allIds.length ? new Set() : new Set(allIds)));
  }, [allIds]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={busy ? () => {} : onClose}
      title={title}
      size="medium"
      closeOnOverlayClick={!busy}
    >
      <div data-bf-component="group-member-picker-dialog" data-bf-part="root" className="group-chat-dialog">
        <div className="group-chat-dialog__members">
          <div className="group-chat-dialog__members-header">
            <span className="group-chat-dialog__members-label">{t('nav.groupChats.members')}</span>
            {sessions.length > 0 ? (
              <Checkbox
                checked={allSelected}
                onChange={toggleSelectAll}
                label={allSelected ? t('actions.deselectAll') : t('actions.selectAll')}
                size="small"
              />
            ) : null}
          </div>

          {isLoading ? (
            <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
          ) : loadFailed ? (
            <div className="group-chat-dialog__state">
              {t('nav.groupChats.membersLoadFailed')}
              <Button type="button" variant="secondary" size="small" onClick={() => { void loadSessions(); }}>
                {t('actions.retry')}
              </Button>
            </div>
          ) : sessions.length === 0 ? (
            <div className="group-chat-dialog__state">{t('nav.groupChats.noClawSessions')}</div>
          ) : (
            <div className="group-chat-dialog__member-list">
              {sessions.map(meta => {
                const isSelected = selectedIds.has(meta.sessionId);
                return (
                  <label
                    key={meta.sessionId}
                    className={`group-chat-dialog__member-row${isSelected ? ' is-selected' : ''}`}
                  >
                    <Checkbox
                      checked={isSelected}
                      onChange={() => toggle(meta.sessionId)}
                      disabled={busy}
                    />
                    <span className="group-chat-dialog__member-name">
                      {meta.sessionName || t('nav.sessions.untitled')}
                    </span>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => { void onConfirm(Array.from(selectedIds)); }}
            disabled={busy || selectedIds.size === 0}
            isLoading={busy}
          >
            {t('nav.groupChats.confirmInvite')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

/**
 * R-GC-15: fork dialog — child group name + member multi-select. Reuses the
 * same Input (Input.tsx:20) + Checkbox multi-select shape as the create-group
 * dialog; no new picker is built.
 */
interface GroupForkDialogProps {
  groupName?: string;
  workspacePath: string;
  isOpen: boolean;
  busy: boolean;
  onClose: () => void;
  onConfirm: (name: string, memberIds: string[]) => void | Promise<void>;
}

function GroupForkDialog({
  groupName,
  workspacePath,
  isOpen,
  busy,
  onClose,
  onConfirm,
}: GroupForkDialogProps) {
  const { t } = useI18n('common');
  const [name, setName] = useState('');
  const [sessions, setSessions] = useState<SessionMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const loadSessions = useCallback(() => {
    setIsLoading(true);
    setLoadFailed(false);
    return sessionAPI
      .listSessions(workspacePath)
      .then(list => setSessions(list.filter(meta => meta.agentType === 'Claw')))
      .catch(error => {
        log.warn('Failed to load Claw sessions for fork dialog', { error, workspacePath });
        setLoadFailed(true);
      })
      .finally(() => setIsLoading(false));
  }, [workspacePath]);

  // Only seed the default child-group name when the dialog opens; never reset
  // the user's typed name on unrelated re-renders (t/groupName excluded from
  // deps for that reason).
  const forkOpenedRef = React.useRef(false);
  useEffect(() => {
    if (!isOpen) {
      forkOpenedRef.current = false;
      setName('');
      setSelectedIds(new Set());
      setLoadFailed(false);
      return;
    }
    if (!forkOpenedRef.current) {
      forkOpenedRef.current = true;
      setName(`${groupName || t('nav.groupChats.untitled')} ${t('nav.groupChats.forkSuffix')}`);
    }
    void loadSessions();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, loadSessions]);

  const toggle = useCallback((sessionId: string) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(sessionId)) {
        next.delete(sessionId);
      } else {
        next.add(sessionId);
      }
      return next;
    });
  }, []);

  const allIds = sessions.map(meta => meta.sessionId);
  const allSelected = allIds.length > 0 && selectedIds.size === allIds.length;

  const toggleSelectAll = useCallback(() => {
    setSelectedIds(prev => (prev.size === allIds.length ? new Set() : new Set(allIds)));
  }, [allIds]);

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

        <div className="group-chat-dialog__members">
          <div className="group-chat-dialog__members-header">
            <span className="group-chat-dialog__members-label">{t('nav.groupChats.members')}</span>
            {sessions.length > 0 ? (
              <Checkbox
                checked={allSelected}
                onChange={toggleSelectAll}
                label={allSelected ? t('actions.deselectAll') : t('actions.selectAll')}
                size="small"
              />
            ) : null}
          </div>

          {isLoading ? (
            <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
          ) : loadFailed ? (
            <div className="group-chat-dialog__state">
              {t('nav.groupChats.membersLoadFailed')}
              <Button type="button" variant="secondary" size="small" onClick={() => { void loadSessions(); }}>
                {t('actions.retry')}
              </Button>
            </div>
          ) : sessions.length === 0 ? (
            <div className="group-chat-dialog__state">{t('nav.groupChats.noClawSessions')}</div>
          ) : (
            <div className="group-chat-dialog__member-list">
              {sessions.map(meta => {
                const isSelected = selectedIds.has(meta.sessionId);
                return (
                  <label
                    key={meta.sessionId}
                    className={`group-chat-dialog__member-row${isSelected ? ' is-selected' : ''}`}
                  >
                    <Checkbox
                      checked={isSelected}
                      onChange={() => toggle(meta.sessionId)}
                      disabled={busy}
                    />
                    <span className="group-chat-dialog__member-name">
                      {meta.sessionName || t('nav.sessions.untitled')}
                    </span>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => { void onConfirm(trimmedName, Array.from(selectedIds)); }}
            disabled={busy || !trimmedName}
            isLoading={busy}
          >
            {t('nav.groupChats.confirmFork')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default GroupChatView;

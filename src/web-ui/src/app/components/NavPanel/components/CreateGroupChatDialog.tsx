/**
 * CreateGroupChatDialog — group chat create dialog (R-GC-13).
 *
 * Reuse rules:
 * - Modal / Button / Input / Checkbox all from component-library (existing components).
 * - Member multi-select reuses the WorkspaceSessionBatchModal "session list +
 *   Checkbox multi-select" shape
 *   (src/web-ui/src/app/components/NavPanel/sections/workspaces/WorkspaceSessionBatchModal.tsx:305-459);
 *   no new picker is built.
 * - Create goes through toolAPI.executeTool (camelCase — the only existing
 *   execute_tool wrapper, ToolAPI.ts:49-61); direct invoke('create_group_chat')
 *   is forbidden (the backend command was removed in R-GC-05).
 */

import React, { useCallback, useEffect, useState } from 'react';
import { Button, Checkbox, Input, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionMetadata } from '@/shared/types/session-history';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';
import './CreateGroupChatDialog.scss';

const log = createLogger('CreateGroupChatDialog');

interface CreateGroupChatDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Current workspace rootPath (contract section 1.2a: workspace_path = workspaceManager rootPath). */
  workspacePath: string;
  onCreated: (groupId: string, name: string) => void | Promise<void>;
}

export const CreateGroupChatDialog: React.FC<CreateGroupChatDialogProps> = ({
  isOpen,
  onClose,
  workspacePath,
  onCreated,
}) => {
  const { t } = useI18n('common');
  const [name, setName] = useState('');
  const [members, setMembers] = useState<SessionMetadata[]>([]);
  const [selectedMemberIds, setSelectedMemberIds] = useState<Set<string>>(new Set());
  const [isLoadingMembers, setIsLoadingMembers] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Member data source = existing session list (listSessions), filtered by
  // agentType === 'Claw' (contract section 1.2).
  const loadMembers = useCallback(async () => {
    setIsLoadingMembers(true);
    setLoadFailed(false);
    try {
      const list = await sessionAPI.listSessions(workspacePath);
      setMembers(list.filter(meta => meta.agentType === 'Claw'));
    } catch (error) {
      log.warn('Failed to load Claw sessions for group member picker', { error, workspacePath });
      setLoadFailed(true);
    } finally {
      setIsLoadingMembers(false);
    }
  }, [workspacePath]);

  useEffect(() => {
    if (!isOpen) {
      setName('');
      setSelectedMemberIds(new Set());
      setLoadFailed(false);
      return;
    }
    void loadMembers();
  }, [isOpen, loadMembers]);

  const toggleMember = useCallback((sessionId: string) => {
    setSelectedMemberIds(prev => {
      const next = new Set(prev);
      if (next.has(sessionId)) {
        next.delete(sessionId);
      } else {
        next.add(sessionId);
      }
      return next;
    });
  }, []);

  const allMemberIds = members.map(meta => meta.sessionId);
  const allSelected = allMemberIds.length > 0 && selectedMemberIds.size === allMemberIds.length;

  const toggleSelectAll = useCallback(() => {
    setSelectedMemberIds(prev => (
      prev.size === allMemberIds.length ? new Set() : new Set(allMemberIds)
    ));
  }, [allMemberIds]);

  const handleCreate = useCallback(async () => {
    const trimmedName = name.trim();
    if (!trimmedName || isSubmitting) return;
    setIsSubmitting(true);
    try {
      const memberIds = Array.from(selectedMemberIds);
      // Contract section 1.4: go through execute_tool (ToolAPI camelCase
      // wrapper); direct invoke('create_group_chat') is forbidden.
      const response = await toolAPI.executeTool({
        toolName: 'create_group_chat',
        parameters: { action: 'create', name: trimmedName, members: memberIds },
        workspacePath,
      });
      const groupId = response?.result?.groupId;
      if (response?.success !== true || typeof groupId !== 'string' || !groupId) {
        const message =
          response?.error ||
          response?.validation_error ||
          t('nav.groupChats.createFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      notificationService.success(t('nav.groupChats.created', { name: trimmedName }), { duration: 3000 });
      await onCreated(groupId, trimmedName);
      onClose();
    } catch (error) {
      log.error('Failed to create group chat', { error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.createFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsSubmitting(false);
    }
  }, [isSubmitting, name, onClose, onCreated, selectedMemberIds, t, workspacePath]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={isSubmitting ? () => {} : onClose}
      title={t('nav.groupChats.newGroupChat')}
      size="medium"
      closeOnOverlayClick={!isSubmitting}
    >
      <div data-bf-component="create-group-chat-dialog" data-bf-part="root" className="group-chat-dialog">
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
            {members.length > 0 ? (
              <Checkbox
                checked={allSelected}
                onChange={toggleSelectAll}
                label={allSelected ? t('actions.deselectAll') : t('actions.selectAll')}
                size="small"
              />
            ) : null}
          </div>

          {isLoadingMembers ? (
            <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
          ) : loadFailed ? (
            <div className="group-chat-dialog__state">
              {t('nav.groupChats.membersLoadFailed')}
              <Button type="button" variant="secondary" size="small" onClick={() => { void loadMembers(); }}>
                {t('actions.retry')}
              </Button>
            </div>
          ) : members.length === 0 ? (
            <div className="group-chat-dialog__state">{t('nav.groupChats.noClawSessions')}</div>
          ) : (
            <div className="group-chat-dialog__member-list">
              {members.map(meta => {
                const isSelected = selectedMemberIds.has(meta.sessionId);
                return (
                  <label
                    key={meta.sessionId}
                    className={`group-chat-dialog__member-row${isSelected ? ' is-selected' : ''}`}
                  >
                    <Checkbox
                      checked={isSelected}
                      onChange={() => toggleMember(meta.sessionId)}
                      disabled={isSubmitting}
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
          <Button type="button" variant="ghost" onClick={onClose} disabled={isSubmitting}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => { void handleCreate(); }}
            disabled={!name.trim() || isSubmitting}
            isLoading={isSubmitting}
          >
            {t('nav.groupChats.create')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default CreateGroupChatDialog;

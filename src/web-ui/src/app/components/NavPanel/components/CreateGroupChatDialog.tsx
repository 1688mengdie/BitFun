/**
 * CreateGroupChatDialog - group chat create dialog (R-GC-13 / R-GC-28).
 *
 * Reuse rules:
 * - Modal / Button / Input all from component-library (existing components).
 * - R-GC-28 (owner directive): members are always CREATED fresh (unique UUID +
 *   default Claw agent type + Claw name + default workspace), never reusing
 *   existing sessions. The picker therefore no longer lists existing Claw
 *   sessions (list_sessions is forbidden as a member source); instead the user
 *   enters the number of members to create. The backend creates N fresh member
 *   sessions (group_room_tools.rs create_member_session).
 * - Create goes through toolAPI.executeTool (camelCase - the only existing
 *   execute_tool wrapper, ToolAPI.ts:49-61); direct invoke('create_group_chat')
 *   is forbidden (the backend command was removed in R-GC-05).
 */

import React, { useCallback, useEffect, useState } from 'react';
import { Button, Input, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';
import './CreateGroupChatDialog.scss';

const log = createLogger('CreateGroupChatDialog');

const MAX_MEMBER_COUNT = 20;

interface CreateGroupChatDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Group workspace rootPath (R-GC-26: Claw default assistant workspace). */
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
  const [memberCount, setMemberCount] = useState(0);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      setName('');
      setMemberCount(0);
      return;
    }
  }, [isOpen]);

  const parsedMemberCount = Number.isFinite(memberCount)
    ? Math.max(0, Math.min(MAX_MEMBER_COUNT, Math.floor(memberCount)))
    : 0;

  const handleCreate = useCallback(async () => {
    const trimmedName = name.trim();
    if (!trimmedName || isSubmitting) return;
    setIsSubmitting(true);
    try {
      // R-GC-28: members are placeholders only (count-driven); the backend
      // creates N fresh unique-UUID member sessions. Never reuse existing ids.
      const memberIds = Array.from({ length: parsedMemberCount }, (_, i) => `member-${i + 1}`);
      // Contract section 1.4: go through execute_tool (ToolAPI camelCase
      // wrapper); direct invoke('create_group_chat') is forbidden.
      const response = await toolAPI.executeTool({
        toolName: 'create_group_chat',
        parameters: { action: 'create', name: trimmedName, members: memberIds, workspace: workspacePath || undefined },
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
  }, [isSubmitting, name, onClose, onCreated, parsedMemberCount, t, workspacePath]);

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

        <div className="group-chat-dialog__field">
          <Input
            label={t('nav.groupChats.memberCount')}
            type="number"
            min={0}
            max={MAX_MEMBER_COUNT}
            value={memberCount}
            onChange={e => setMemberCount(Number(e.target.value))}
            inputSize="medium"
          />
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

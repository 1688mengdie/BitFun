/**
 * FlowChat header.
 * Shows the currently viewed turn and user message.
 * Height matches side panel headers (40px).
 */

import React from 'react';
import { CornerUpLeft, MessageSquarePlus } from 'lucide-react';
import { Tooltip, IconButton } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { globalEventBus } from '@/infrastructure/event-bus';
import { SessionFilesBadge } from './SessionFilesBadge';
import type { Session } from '../../types/flow-chat';
import './FlowChatHeader.scss';

export interface FlowChatHeaderProps {
  /** Current turn index. */
  currentTurn: number;
  /** Total turns. */
  totalTurns: number;
  /** Current user message. */
  currentUserMessage: string;
  /** Whether the header is visible. */
  visible: boolean;
  /** Session ID. */
  sessionId?: string;
  /** BTW child-session origin metadata. */
  btwOrigin?: Session['btwOrigin'] | null;
  /** BTW parent session title. */
  btwParentTitle?: string;
  /** Creates a new BTW thread from the current session. */
  onCreateBtwSession?: () => void;
}
export const FlowChatHeader: React.FC<FlowChatHeaderProps> = ({
  currentTurn,
  totalTurns,
  currentUserMessage,
  visible,
  sessionId,
  btwOrigin,
  btwParentTitle = '',
  onCreateBtwSession,
}) => {
  const { t } = useTranslation('flow-chat');

  if (!visible || totalTurns === 0) {
    return null;
  }

  // Truncate long messages.
  const truncatedMessage = currentUserMessage.length > 50
    ? currentUserMessage.slice(0, 50) + '...'
    : currentUserMessage;
  const parentLabel = btwParentTitle || t('btw.parent', { defaultValue: 'parent session' });
  const backTooltip = btwOrigin?.parentTurnIndex
    ? t('flowChatHeader.btwBackTooltipWithTurn', {
      title: parentLabel,
      turn: btwOrigin.parentTurnIndex,
      defaultValue: `Go back to the source session: ${parentLabel} (Turn ${btwOrigin.parentTurnIndex})`,
    })
    : t('flowChatHeader.btwBackTooltipWithoutTurn', {
      title: parentLabel,
      defaultValue: `Go back to the source session: ${parentLabel}`,
    });
  const createBtwTooltip = t('flowChatHeader.btwCreateTooltip', {
    defaultValue: 'Start a quick side question',
  });
  const turnBadgeLabel = t('flowChatHeader.turnBadge', {
    current: currentTurn,
    defaultValue: `Turn ${currentTurn}`,
  });

  const handleBackToParent = () => {
    const parentId = btwOrigin?.parentSessionId;
    if (!parentId) return;
    const requestId = btwOrigin?.requestId;
    const itemId = requestId ? `btw_marker_${requestId}` : undefined;
    globalEventBus.emit('flowchat:focus-item', {
      sessionId: parentId,
      turnIndex: btwOrigin?.parentTurnIndex,
      itemId,
    }, 'FlowChatHeader');
  };

  return (
    <div className="flowchat-header">
      <div className="flowchat-header__actions flowchat-header__actions--left">
        <SessionFilesBadge sessionId={sessionId} />
      </div>

      <Tooltip content={currentUserMessage} placement="bottom">
        <div className="flowchat-header__message">
          <span className="flowchat-header__turn-badge" aria-label={turnBadgeLabel}>
            <span>{turnBadgeLabel}</span>
          </span>
          <span className="flowchat-header__message-text">
            {truncatedMessage}
          </span>
        </div>
      </Tooltip>

      <div className="flowchat-header__actions">
        {!!btwOrigin?.parentSessionId && (
          <IconButton
            className="flowchat-header__btw-back"
            variant="ghost"
            size="xs"
            onClick={handleBackToParent}
            tooltip={backTooltip}
            disabled={!btwOrigin.parentSessionId}
            aria-label={t('btw.back', { defaultValue: 'Back' })}
            data-testid="flowchat-header-btw-back"
          >
            <CornerUpLeft size={12} />
          </IconButton>
        )}
        {onCreateBtwSession && (
          <IconButton
            className="flowchat-header__btw-create"
            variant="ghost"
            size="xs"
            onClick={onCreateBtwSession}
            tooltip={createBtwTooltip}
            aria-label={createBtwTooltip}
            data-testid="flowchat-header-btw-create"
          >
            <MessageSquarePlus size={14} />
          </IconButton>
        )}
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';


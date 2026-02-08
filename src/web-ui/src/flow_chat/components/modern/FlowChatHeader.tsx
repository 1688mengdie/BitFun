/**
 * FlowChat header.
 * Shows the currently viewed turn and user message.
 * Height matches side panel headers (40px).
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronUp, ChevronDown } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { SessionFilesBadge } from './SessionFilesBadge';
import './FlowChatHeader.scss';

export interface FlowChatHeaderProps {
  /** Current visible turn index (1-based). */
  currentTurnIndex: number;
  /** Total turns. */
  totalTurns: number;
  /** Current user message. */
  currentUserMessage: string;
  /** Whether the header is visible. */
  visible: boolean;
  /** Previous turn callback. */
  onPreviousTurn?: () => void;
  /** Next turn callback. */
  onNextTurn?: () => void;
  /** Session ID. */
  sessionId?: string;
}
export const FlowChatHeader: React.FC<FlowChatHeaderProps> = ({
  currentTurnIndex,
  totalTurns,
  currentUserMessage,
  visible,
  onPreviousTurn,
  onNextTurn,
  sessionId,
}) => {
  const { t } = useTranslation('flow-chat');

  if (!visible || totalTurns === 0) {
    return null;
  }

  // Truncate long messages.
  const truncatedMessage = currentUserMessage.length > 50
    ? currentUserMessage.slice(0, 50) + '...'
    : currentUserMessage;

  return (
    <div className="flowchat-header">
      <div className="flowchat-header__actions flowchat-header__actions--left">
        <SessionFilesBadge sessionId={sessionId} />
      </div>

      <Tooltip content={currentUserMessage} placement="bottom">
        <div className="flowchat-header__message">
          {truncatedMessage}
        </div>
      </Tooltip>

      <div className="flowchat-header__actions">
        <span className="flowchat-header__turn-info">
          {currentTurnIndex} / {totalTurns}
        </span>
        <Tooltip content={t('flowChatHeader.previousTurn')}>
          <button
            className="flowchat-header__nav-btn"
            onClick={onPreviousTurn}
            disabled={currentTurnIndex <= 1}
          >
            <ChevronUp size={16} />
          </button>
        </Tooltip>
        <Tooltip content={t('flowChatHeader.nextTurn')}>
          <button
            className="flowchat-header__nav-btn"
            onClick={onNextTurn}
            disabled={currentTurnIndex >= totalTurns}
          >
            <ChevronDown size={16} />
          </button>
        </Tooltip>
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';


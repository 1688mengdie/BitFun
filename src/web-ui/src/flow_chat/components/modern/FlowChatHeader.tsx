/**
 * FlowChat header.
 * Shows the currently viewed turn and user message.
 * Height matches side panel headers (40px).
 */

import React from 'react';
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
  /** Session ID. */
  sessionId?: string;
}
export const FlowChatHeader: React.FC<FlowChatHeaderProps> = ({
  currentTurnIndex,
  totalTurns,
  currentUserMessage,
  visible,
  sessionId,
}) => {
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
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';


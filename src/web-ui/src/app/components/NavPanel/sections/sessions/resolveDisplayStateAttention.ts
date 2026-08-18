/**
 * R-WF-11 / R-12: derive the row notification dot from the backend seven-state
 * `displayState` projection when the event-driven unread markers
 * (`needsUserAttention` / `hasUnreadCompletion`) are absent. This closes the
 * gap for historical sessions restored purely from `SessionMetadata`, where
 * the local runtime never emitted an unread event.
 */
import type { Session } from '../../../../../flow_chat/types/flow-chat';
import { SessionDisplayState } from '../../../../../flow_chat/state-machine/types';

export type DisplayStateAttentionKind =
  | 'error'
  | 'interrupted'
  | 'completed'
  | 'ask_user'
  | 'tool_confirm'
  | undefined;

export const resolveDisplayStateAttention = (
  session: Session,
): DisplayStateAttentionKind => {
  const displayState = session.displayState;
  if (!displayState) return undefined;
  switch (displayState) {
    case SessionDisplayState.PENDING_ATTENTION:
      return 'ask_user';
    case SessionDisplayState.INTERRUPTED:
      return 'interrupted';
    case SessionDisplayState.PROCESSING:
    case SessionDisplayState.HUNG:
    case SessionDisplayState.VIEWED:
      return undefined;
    case SessionDisplayState.COMPLETED:
      return 'completed';
    case SessionDisplayState.STANDBY:
      return undefined;
    default:
      return undefined;
  }
};

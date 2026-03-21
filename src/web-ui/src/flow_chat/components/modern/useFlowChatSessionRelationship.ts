/**
 * Derived BTW/session relationship state for FlowChat header UI.
 */

import { useEffect, useMemo, useState } from 'react';
import { flowChatStore } from '../../store/FlowChatStore';
import type { Session } from '../../types/flow-chat';
import { resolveSessionRelationship } from '../../utils/sessionMetadata';

interface UseFlowChatSessionRelationshipResult {
  isBtwSession: boolean;
  btwOrigin: Session['btwOrigin'] | null;
  btwParentTitle: string;
}

export function useFlowChatSessionRelationship(
  activeSession: Session | null,
): UseFlowChatSessionRelationshipResult {
  const [btwOrigin, setBtwOrigin] = useState<Session['btwOrigin'] | null>(null);
  const [btwParentTitle, setBtwParentTitle] = useState('');

  const isBtwSession = useMemo(() => {
    return resolveSessionRelationship(activeSession).isBtw;
  }, [activeSession]);

  useEffect(() => {
    const syncRelationshipState = (state = flowChatStore.getState()) => {
      const currentSessionId = activeSession?.sessionId;
      if (!currentSessionId) {
        setBtwOrigin(null);
        setBtwParentTitle('');
        return;
      }

      const session = state.sessions.get(currentSessionId);
      if (!session) {
        setBtwOrigin(null);
        setBtwParentTitle('');
        return;
      }

      const relationship = resolveSessionRelationship(session);
      const nextOrigin = relationship.origin || null;
      const parentId = relationship.parentSessionId;
      const parent = parentId ? state.sessions.get(parentId) : undefined;

      setBtwOrigin(nextOrigin);
      setBtwParentTitle(parent?.title || '');
    };

    syncRelationshipState();
    const unsubscribe = flowChatStore.subscribe(syncRelationshipState);
    return unsubscribe;
  }, [activeSession?.sessionId]);

  return {
    isBtwSession,
    btwOrigin,
    btwParentTitle,
  };
}

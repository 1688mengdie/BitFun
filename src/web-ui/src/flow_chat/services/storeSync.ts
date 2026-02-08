/**
 * Store sync service
 * Syncs data from old FlowChatStore to new ModernFlowChatStore
 * Maintains original concept: Session → DialogTurn → ModelRound → FlowItem
 */

import { flowChatStore } from '../store/FlowChatStore';
import { useModernFlowChatStore } from '../store/modernFlowChatStore';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('StoreSync');

/**
 * Sync session data to new Store
 */
export function syncSessionToModernStore(sessionId: string): void {
  const oldState = flowChatStore.getState();
  const session = oldState.sessions.get(sessionId);

  if (!session) {
    log.warn('Session not found', { sessionId });
    return;
  }

  const modernStore = useModernFlowChatStore.getState();
  modernStore.setActiveSession(session);
  
  setTimeout(() => {
    modernStore.updateVirtualItems();
  }, 0);
}

/**
 * Start auto sync
 * Listens to old Store changes and automatically syncs to new Store
 *
 * Performance optimization: relies on FlowChatStore's immutable updates, each update creates a new session reference
 */
export function startAutoSync(): () => void {
  const unsubscribe = flowChatStore.subscribe((state) => {
    const modernStore = useModernFlowChatStore.getState();
    
    if (state.activeSessionId) {
      const session = state.sessions.get(state.activeSessionId);
      if (session) {
        modernStore.setActiveSession(session);
      }
    } else {
      modernStore.clear();
    }
  });
  
  const currentState = flowChatStore.getState();
  if (currentState.activeSessionId) {
    const session = currentState.sessions.get(currentState.activeSessionId);
    if (session) {
      const modernStore = useModernFlowChatStore.getState();
      modernStore.setActiveSession(session);
    }
  }
  
  return unsubscribe;
}
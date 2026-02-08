/**
 * Infrastructure unified exports.
 */

// Event bus
export * from './event-bus';

// State management
export * from './state-management';

// API layer
export * from './api';

// Contexts (explicit exports to avoid name collisions)
export { ChatProvider, useChat } from './contexts/ChatContext';
export { 
  WorkspaceProvider, 
  useWorkspaceContext,
  // Renamed to avoid collisions with state-management exports.
  useCurrentWorkspace as useCurrentWorkspaceFromContext 
} from './contexts/WorkspaceContext';

// Configuration
export * from './config';

// Infrastructure hooks (explicit exports to avoid name collisions)
export { useWorkspace } from './hooks/useWorkspace';
export * from './hooks/useAIInitialization';

// Infrastructure lifecycle
import { initializeGlobalState } from './state-management';
import { initializeConfigInfrastructure } from './config';
import { globalEventBus } from './event-bus';

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('Infrastructure');

export async function initializeInfrastructure(): Promise<void> {
  log.info('Initializing infrastructure systems');
  
  try {
    // Initialize global state
    initializeGlobalState();
    
    // Initialize configuration infrastructure
    await initializeConfigInfrastructure();
    
    // Notify that infrastructure is ready
    globalEventBus.emit('infrastructure:ready');
    
    log.info('Infrastructure systems initialized successfully');
  } catch (error) {
    log.error('Failed to initialize infrastructure systems', error);
    throw error;
  }
}

export async function destroyInfrastructure(): Promise<void> {
  log.info('Shutting down infrastructure systems');
  
  // Notify shutdown
  globalEventBus.emit('infrastructure:shutdown');
  
  // Destroy event bus last
  globalEventBus.destroy();
}

// Backward-compatible aliases
export const initializeCore = initializeInfrastructure;
export const destroyCore = destroyInfrastructure;

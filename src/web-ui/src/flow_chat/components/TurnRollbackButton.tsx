import React, { useState } from 'react';
import { snapshotAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './TurnRollbackButton.scss';

const log = createLogger('TurnRollbackButton');

interface TurnRollbackButtonProps {
  sessionId: string;
  turnIndex: number;
  isCurrent: boolean;
  onRollbackComplete?: () => void;
}

export const TurnRollbackButton: React.FC<TurnRollbackButtonProps> = ({
  sessionId,
  turnIndex,
  isCurrent,
  onRollbackComplete,
}) => {
  const [loading, setLoading] = useState(false);
  
  const handleRollback = async () => {
    if (isCurrent || loading) return;
    
    // In Tauri, window.confirm is async and must be awaited.
    const confirmed = await window.confirm(
      `Roll back to before turn ${turnIndex + 1}?\n\n` +
      `This will:\n` +
      `• Restore files to the state before turn ${turnIndex + 1}\n` +
      `• Undo all file changes from turn ${turnIndex + 1} onward\n` +
      `• Keep the conversation history (you can roll forward again anytime)`
    );
    
    if (!confirmed) return;
    
    setLoading(true);
    try {
      const restoredFiles = await snapshotAPI.rollbackToTurn(sessionId, turnIndex);
      
      log.debug('Rollback completed', { sessionId, turnIndex, restoredFilesCount: restoredFiles.length });
      
      // Notify related components to refresh.
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      
      // Refresh file tree.
      globalEventBus.emit('file-tree:refresh');
      
      // Refresh open files in the editor.
      restoredFiles.forEach(filePath => {
        globalEventBus.emit('editor:file-changed', { filePath });
      });
      
      // Refresh snapshot state.
      globalEventBus.emit('snapshot:rollback-completed', { 
        sessionId,
        turnIndex,
        restoredFiles
      });
      
      // Notify parent.
      if (onRollbackComplete) {
        onRollbackComplete();
      }
      
    } catch (error) {
      log.error('Rollback failed', { sessionId, turnIndex, error });
      notificationService.error(`Rollback failed: ${error}`);
    } finally {
      setLoading(false);
    }
  };
  
  if (isCurrent) {
    return <span className="turn-rollback-button-current">Current</span>;
  }
  
  return (
    <button
      className="turn-rollback-button"
      onClick={handleRollback}
      disabled={loading}
      title={`Rollback to turn ${turnIndex + 1}`}
    >
      {loading ? 'Rolling back...' : 'Rollback'}
    </button>
  );
};



 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SnapshotAPI');


export interface SandboxSessionModifications {
  hasModifications: boolean;
  totalFiles: number;
  totalAdditions: number;
  totalDeletions: number;
  modifiedFiles: Array<{
    filePath: string;
    toolName: string;
    operationType: string;
    additions: number;
    deletions: number;
  }>;
}

export interface SandboxOperationDiff {
  filePath: string;
  originalContent: string;
  modifiedContent: string;
  diff?: string;
  operationType?: string;
  toolName?: string;
  anchorLine?: number | null;
}

export interface GetSessionModificationsRequest {
  sessionId: string;
}

export interface GetOperationDiffRequest {
  sessionId: string;
  filePath: string;
  operationId?: string;
}

export interface GetBaselineSnapshotDiffRequest {
  filePath: string;
}

export interface SandboxOperationSummary {
  operationId: string;
  sessionId: string;
  turnIndex?: number | null;
  seqInTurn?: number | null;
  filePath?: string | null;
  operationType?: string | null;
  toolName?: string | null;
  linesAdded?: number | null;
  linesRemoved?: number | null;
}

export interface GetOperationSummaryRequest {
  sessionId: string;
  operationId: string;
}

export interface AcceptSessionModificationsRequest {
  sessionId: string;
}

export interface RejectSessionModificationsRequest {
  sessionId: string;
}

export interface AcceptFileModificationsRequest {
  sessionId: string;
  filePath: string;
}

export interface RejectFileModificationsRequest {
  sessionId: string;
  filePath: string;
}

export interface AcceptDiffBlockRequest {
  sessionId: string;
  filePath: string;
  blockIndex: number;
}

export interface RejectDiffBlockRequest {
  sessionId: string;
  filePath: string;
  blockIndex: number;
}

export interface AcceptOperationRequest {
  sessionId: string;
  operationId: string;
}

export interface RejectOperationRequest {
  sessionId: string;
  operationId: string;
}

export interface RollbackSessionRequest {
  sessionId: string;
}

export interface CleanupSandboxDataRequest {
  maxAgeDays: number;
}

export class SnapshotAPI {
   
  async getSessionStats(sessionId: string): Promise<{
    session_id: string;
    total_files: number;
    total_turns: number;
    total_changes: number;
  }> {
    try {
      return await api.invoke('get_session_stats', { 
        request: { session_id: sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('get_session_stats', error, { sessionId });
    }
  }

   
  async getSessionFiles(sessionId: string): Promise<string[]> {
    try {
      return await api.invoke('get_session_files', { 
        request: { session_id: sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('get_session_files', error, { sessionId });
    }
  }

   
  async getOperationDiff(sessionId: string, filePath: string, operationId?: string): Promise<SandboxOperationDiff> {
    try {
      return await api.invoke('get_operation_diff', { 
        request: { sessionId, filePath, operationId } 
      });
    } catch (error) {
      throw createTauriCommandError('get_operation_diff', error, { sessionId, filePath, operationId });
    }
  }

  async getOperationSummary(sessionId: string, operationId: string): Promise<SandboxOperationSummary> {
    try {
      return await api.invoke('get_operation_summary', {
        request: { sessionId, operationId }
      });
    } catch (error) {
      throw createTauriCommandError('get_operation_summary', error, { sessionId, operationId });
    }
  }

   
  async getBaselineSnapshotDiff(filePath: string): Promise<SandboxOperationDiff> {
    try {
      return await api.invoke('get_baseline_snapshot_diff', {
        request: { filePath }
      });
    } catch (error) {
      throw createTauriCommandError('get_baseline_snapshot_diff', error, { filePath });
    }
  }



   
  async acceptSessionModifications(sessionId: string): Promise<void> {
    try {
      await api.invoke('accept_session_modifications', { 
        request: { sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('accept_session_modifications', error, { sessionId });
    }
  }

   
  async rejectSessionModifications(sessionId: string): Promise<void> {
    try {
      await api.invoke('reject_session_modifications', { 
        request: { sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('reject_session_modifications', error, { sessionId });
    }
  }

   
  async acceptFileModifications(sessionId: string, filePath: string): Promise<void> {
    try {
      await api.invoke('accept_file_modifications', { 
        request: { sessionId, filePath } 
      });
    } catch (error) {
      throw createTauriCommandError('accept_file_modifications', error, { sessionId, filePath });
    }
  }

   
  async rejectFileModifications(sessionId: string, filePath: string): Promise<void> {
    try {
      await api.invoke('reject_file_modifications', { 
        request: { sessionId, filePath } 
      });
    } catch (error) {
      throw createTauriCommandError('reject_file_modifications', error, { sessionId, filePath });
    }
  }

   
  async acceptDiffBlock(sessionId: string, filePath: string, blockIndex: number): Promise<void> {
    try {
      await api.invoke('accept_diff_block', { 
        request: { sessionId, filePath, blockId: blockIndex.toString() } 
      });
    } catch (error) {
      throw createTauriCommandError('accept_diff_block', error, { sessionId, filePath, blockIndex });
    }
  }

   
  async rejectDiffBlock(sessionId: string, filePath: string, blockIndex: number): Promise<void> {
    try {
      await api.invoke('reject_diff_block', { 
        request: { sessionId, filePath, blockId: blockIndex.toString() } 
      });
    } catch (error) {
      throw createTauriCommandError('reject_diff_block', error, { sessionId, filePath, blockIndex });
    }
  }

   
  async acceptOperation(sessionId: string, operationId: string): Promise<void> {
    try {
      await api.invoke('accept_operation', { 
        request: { sessionId, operationId } 
      });
    } catch (error) {
      throw createTauriCommandError('accept_operation', error, { sessionId, operationId });
    }
  }

   
  async rejectOperation(sessionId: string, operationId: string): Promise<void> {
    try {
      await api.invoke('reject_operation', { 
        request: { sessionId, operationId } 
      });
    } catch (error) {
      throw createTauriCommandError('reject_operation', error, { sessionId, operationId });
    }
  }

   
  async rollbackSession(sessionId: string): Promise<void> {
    try {
      await api.invoke('rollback_session', { 
        request: { sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('rollback_session', error, { sessionId });
    }
  }

   
  async cleanupSnapshotData(maxAgeDays: number = 30): Promise<any> {
    try {
      return await api.invoke('cleanup_snapshot_data', {
        request: { maxAgeDays } 
      });
    } catch (error) {
      throw createTauriCommandError('cleanup_snapshot_data', error, { maxAgeDays });
    }
  }

   
  async cleanupEmptySessions(): Promise<any> {
    try {
      return await api.invoke('cleanup_empty_sessions', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('cleanup_empty_sessions', error);
    }
  }

   
  async getSnapshotStats(): Promise<any> {
    try {
      return await api.invoke('get_snapshot_system_stats');
    } catch (error) {
      throw createTauriCommandError('get_snapshot_system_stats', error);
    }
  }

   
  async getSnapshotSessions(): Promise<any> {
    try {
      return await api.invoke('get_snapshot_sessions');
    } catch (error) {
      throw createTauriCommandError('get_snapshot_sessions', error);
    }
  }

   
  async getSessionOperations(sessionId: string): Promise<any> {
    try {
      return await api.invoke('get_session_operations', {
        request: { sessionId }
      });
    } catch (error) {
      throw createTauriCommandError('get_session_operations', error, { sessionId });
    }
  }

  

   
  async recordTurnSnapshot(
    sessionId: string,
    turnIndex: number,
    modifiedFiles: string[]
  ): Promise<void> {
    try {
      await api.invoke('record_turn_snapshot', {
        session_id: sessionId,
        turn_index: turnIndex,
        modified_files: modifiedFiles,
      });
    } catch (error) {
      throw createTauriCommandError('record_turn_snapshot', error, { sessionId, turnIndex, modifiedFiles });
    }
  }

   
  async rollbackToTurn(
    sessionId: string,
    turnIndex: number,
    deleteTurns: boolean = false
  ): Promise<string[]> {
    try {
      return await api.invoke('rollback_to_turn', {
        request: {
          session_id: sessionId,
          turn_index: turnIndex,
          delete_turns: deleteTurns,
        }
      });
    } catch (error) {
      throw createTauriCommandError('rollback_to_turn', error, { sessionId, turnIndex, deleteTurns });
    }
  }

   
  async rollbackEntireSession(
    sessionId: string,
    deleteSession: boolean = true
  ): Promise<string[]> {
    try {
      return await api.invoke('rollback_session', {
        request: {
          session_id: sessionId,
          delete_session: deleteSession,
        }
      });
    } catch (error) {
      throw createTauriCommandError('rollback_session', error, { sessionId });
    }
  }

   
  async getSessionTurnSnapshots(
    sessionId: string
  ): Promise<TurnSnapshot[]> {
    try {
      
      const turnIndices: number[] = await api.invoke('get_session_turns', {
        request: {
          session_id: sessionId,
        }
      });

      
      const turnSnapshots: TurnSnapshot[] = [];
      for (const turnIndex of turnIndices) {
        try {
          const files: string[] = await api.invoke('get_turn_files', {
            request: {
              session_id: sessionId,
              turn_index: turnIndex,
            }
          });

          turnSnapshots.push({
            sessionId,
            turnIndex,
            modifiedFiles: files,
            timestamp: Date.now() / 1000, 
          });
        } catch (error) {
          log.warn('Failed to get turn files', { sessionId, turnIndex, error });
          // Continue processing the remaining turns.
          turnSnapshots.push({
            sessionId,
            turnIndex,
            modifiedFiles: [],
            timestamp: Date.now() / 1000,
          });
        }
      }

      return turnSnapshots;
    } catch (error) {
      throw createTauriCommandError('get_session_turns', error, { sessionId });
    }
  }

   
  async getFileChangeHistory(filePath: string): Promise<FileChangeEntry[]> {
    try {
      const result = await api.invoke('get_file_change_history', {
        request: { file_path: filePath }
      });
      return result as FileChangeEntry[];
    } catch (error) {
      throw createTauriCommandError('get_file_change_history', error, { filePath });
    }
  }

   
  async getAllModifiedFiles(): Promise<string[]> {
    try {
      return await api.invoke('get_all_modified_files', {
        request: {}
      });
    } catch (error) {
      throw createTauriCommandError('get_all_modified_files', error);
    }
  }
}


export interface TurnSnapshot {
  sessionId: string;
  turnIndex: number;
  modifiedFiles: string[];
  timestamp: number;
}


export interface FileChangeEntry {
  session_id: string;
  turn_index: number;
  snapshot_id: string;
  timestamp: {
    secs_since_epoch: number;
    nanos_since_epoch: number;
  };
  operation_type: string;
  tool_name: string;
}


export const snapshotAPI = new SnapshotAPI();

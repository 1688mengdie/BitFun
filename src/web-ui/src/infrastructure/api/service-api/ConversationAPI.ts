 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { SessionMetadata, DialogTurnData } from '@/shared/types/conversation-history';

export class ConversationAPI {
   
  async getConversationSessions(workspacePath: string): Promise<SessionMetadata[]> {
    try {
      return await api.invoke('get_conversation_sessions', {
        request: {
          workspace_path: workspacePath
        }
      });
    } catch (error) {
      throw createTauriCommandError('get_conversation_sessions', error, { workspacePath });
    }
  }

   
  async loadConversationHistory(
    sessionId: string,
    workspacePath: string,
    limit?: number
  ): Promise<DialogTurnData[]> {
    try {
      const request: any = {
        session_id: sessionId,
        workspace_path: workspacePath,
      };
      
      
      if (limit !== undefined) {
        request.limit = limit;
      }
      
      return await api.invoke('load_conversation_history', {
        request
      });
    } catch (error) {
      throw createTauriCommandError('load_conversation_history', error, { sessionId, workspacePath, limit });
    }
  }

   
  async saveDialogTurn(
    turnData: DialogTurnData,
    workspacePath: string
  ): Promise<void> {
    try {
      await api.invoke('save_dialog_turn', {
        request: {
          turn_data: turnData,
          workspace_path: workspacePath
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_dialog_turn', error, { turnData, workspacePath });
    }
  }

   
  async saveSessionMetadata(
    metadata: SessionMetadata,
    workspacePath: string
  ): Promise<void> {
    try {
      await api.invoke('save_session_metadata', {
        request: {
          metadata,
          workspace_path: workspacePath
        }
      });
    } catch (error) {
      throw createTauriCommandError('save_session_metadata', error, { metadata, workspacePath });
    }
  }

   
  async deleteConversationHistory(
    sessionId: string,
    workspacePath: string
  ): Promise<void> {
    try {
      await api.invoke('delete_conversation_history', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath
        }
      });
    } catch (error) {
      throw createTauriCommandError('delete_conversation_history', error, { sessionId, workspacePath });
    }
  }

   
  async touchConversationSession(
    sessionId: string,
    workspacePath: string
  ): Promise<void> {
    try {
      await api.invoke('touch_conversation_session', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath
        }
      });
    } catch (error) {
      throw createTauriCommandError('touch_conversation_session', error, { sessionId, workspacePath });
    }
  }

   
  async loadSessionMetadata(
    sessionId: string,
    workspacePath: string
  ): Promise<SessionMetadata | null> {
    try {
      return await api.invoke('load_session_metadata', {
        request: {
          session_id: sessionId,
          workspace_path: workspacePath
        }
      });
    } catch (error) {
      throw createTauriCommandError('load_session_metadata', error, { sessionId, workspacePath });
    }
  }
}


export const conversationAPI = new ConversationAPI();


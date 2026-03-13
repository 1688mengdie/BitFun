 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

export interface ApplicationState {
  status: AppStatus;
  workspace?: WorkspaceInfo;
  version: string;
  uptime: number;
}

export interface AppStatus {
  isInitialized: boolean;
  hasError: boolean;
  errorMessage?: string;
}

export interface ProjectStatistics {
  totalFiles: number;
  totalLines: number;
  totalSize: number;
  filesByLanguage: Record<string, number>;
  filesByExtension: Record<string, number>;
  lastUpdated: string;
}

export interface WorkspaceIdentity {
  name?: string | null;
  creature?: string | null;
  vibe?: string | null;
  emoji?: string | null;
}

export interface WorkspaceInfo {
  id: string;
  name: string;
  rootPath: string;
  workspaceType: string;
  workspaceKind: string;
  assistantId?: string | null;
  languages: string[];
  openedAt: string;
  lastAccessed: string;
  description?: string | null;
  tags: string[];
  statistics?: ProjectStatistics | null;
  identity?: WorkspaceIdentity | null;
}

export interface UpdateAppStatusRequest {
  status: AppStatus;
}

export interface OpenWorkspaceRequest {
  path: string;
}

export interface CreateAssistantWorkspaceRequest {}

export interface CloseWorkspaceRequest {
  workspaceId: string;
}

export interface SetActiveWorkspaceRequest {
  workspaceId: string;
}

export interface DeleteAssistantWorkspaceRequest {
  workspaceId: string;
}

export interface ResetAssistantWorkspaceRequest {
  workspaceId: string;
}

export interface ScanWorkspaceInfoRequest {
  workspacePath: string;
}

export class GlobalAPI {
   
  async initializeGlobalState(): Promise<string> {
    try {
      return await api.invoke('initialize_global_state', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('initialize_global_state', error);
    }
  }

   
  async getAppState(): Promise<ApplicationState> {
    try {
      return await api.invoke('get_app_state', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_app_state', error);
    }
  }

   
  async updateAppStatus(status: AppStatus): Promise<void> {
    try {
      await api.invoke('update_app_status', { 
        request: { status } 
      });
    } catch (error) {
      throw createTauriCommandError('update_app_status', error, { status });
    }
  }

   
  async openWorkspace(path: string): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('open_workspace', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('open_workspace', error, { path });
    }
  }

  async createAssistantWorkspace(): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('create_assistant_workspace', {
        request: {},
      });
    } catch (error) {
      throw createTauriCommandError('create_assistant_workspace', error);
    }
  }

   
  async closeWorkspace(workspaceId: string): Promise<void> {
    try {
      await api.invoke('close_workspace', { 
        request: { workspaceId } 
      });
    } catch (error) {
      throw createTauriCommandError('close_workspace', error, { workspaceId });
    }
  }

  async setActiveWorkspace(workspaceId: string): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('set_active_workspace', {
        request: { workspaceId }
      });
    } catch (error) {
      throw createTauriCommandError('set_active_workspace', error, { workspaceId });
    }
  }

  async deleteAssistantWorkspace(workspaceId: string): Promise<void> {
    try {
      await api.invoke('delete_assistant_workspace', {
        request: { workspaceId }
      });
    } catch (error) {
      throw createTauriCommandError('delete_assistant_workspace', error, { workspaceId });
    }
  }

  async resetAssistantWorkspace(workspaceId: string): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('reset_assistant_workspace', {
        request: { workspaceId }
      });
    } catch (error) {
      throw createTauriCommandError('reset_assistant_workspace', error, { workspaceId });
    }
  }

   
  async getCurrentWorkspace(): Promise<WorkspaceInfo | null> {
    try {
      return await api.invoke('get_current_workspace', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_current_workspace', error);
    }
  }

   
  async getRecentWorkspaces(): Promise<WorkspaceInfo[]> {
    try {
      return await api.invoke('get_recent_workspaces', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_recent_workspaces', error);
    }
  }

  async getOpenedWorkspaces(): Promise<WorkspaceInfo[]> {
    try {
      return await api.invoke('get_opened_workspaces', {
        request: {}
      });
    } catch (error) {
      throw createTauriCommandError('get_opened_workspaces', error);
    }
  }

   
  async scanWorkspaceInfo(workspacePath: string): Promise<WorkspaceInfo | null> {
    try {
      return await api.invoke('scan_workspace_info', { 
        request: { workspacePath } 
      });
    } catch (error) {
      throw createTauriCommandError('scan_workspace_info', error, { workspacePath });
    }
  }

   
  async getCurrentWorkspacePath(): Promise<string | undefined> {
    try {
      const workspace = await this.getCurrentWorkspace();
      return workspace?.rootPath;
    } catch (error) {
      throw createTauriCommandError('get_current_workspace', error);
    }
  }
}


export const globalAPI = new GlobalAPI();

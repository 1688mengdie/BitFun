/**
 * Subagent API
 */

import { api } from './ApiClient';



export type SubagentSource = 'builtin' | 'project' | 'user';

export interface SubagentInfo {
  id: string;
  name: string;
  description: string;
  isReadonly: boolean;
  toolCount: number;
  defaultTools: string[];
  enabled: boolean;
  subagentSource?: SubagentSource;
  path?: string;
   
  model?: string;
}

export interface ListSubagentsOptions {
  source?: SubagentSource;
}

export type SubagentLevel = 'user' | 'project';

export interface CreateSubagentPayload {
  level: SubagentLevel;
  name: string;
  description: string;
  prompt: string;
  tools?: string[];
   
  readonly?: boolean;
}

export interface UpdateSubagentConfigPayload {
  subagentId: string;
  enabled?: boolean;
  model?: string;
}

// ==================== API ====================

export const SubagentAPI = {
   
  async listSubagents(options?: ListSubagentsOptions): Promise<SubagentInfo[]> {
    return api.invoke<SubagentInfo[]>('list_subagents', {
      request: options ?? {},
    });
  },

   
  async reloadSubagents(): Promise<void> {
    return api.invoke('reload_subagents');
  },

   
  async createSubagent(payload: CreateSubagentPayload): Promise<void> {
    return api.invoke('create_subagent', {
      request: payload,
    });
  },

   
  async listAgentToolNames(): Promise<string[]> {
    return api.invoke<string[]>('list_agent_tool_names');
  },

   
  async updateSubagentConfig(payload: UpdateSubagentConfigPayload): Promise<void> {
    return api.invoke('update_subagent_config', {
      request: payload,
    });
  },
};

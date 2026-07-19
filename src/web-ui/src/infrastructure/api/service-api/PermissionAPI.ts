import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { PermissionReplyKind, PermissionV2Request } from './AgentAPI';

export interface PermissionGrant {
  projectId: string;
  action: string;
  resource: string;
  createdAtMs: number;
}

export type PermissionAuditEvent =
  | { event: 'requested' }
  | { event: 'replied'; reply: { reply: PermissionReplyKind; feedback?: string }; source: 'user' | 'auto_approve' | 'system' }
  | { event: 'cancelled'; reason: string };

export interface PermissionAuditRecord {
  auditId: string;
  request: PermissionV2Request;
  timestampMs: number;
  event: PermissionAuditEvent;
}

export interface PermissionAuditPage {
  projectId: string;
  records: PermissionAuditRecord[];
  page: number;
  pageSize: number;
  total: number;
}

class PermissionAPI {
  async listProjectGrants(workspaceId: string): Promise<PermissionGrant[]> {
    try {
      return await api.invoke<PermissionGrant[]>('list_project_permission_grants', {
        request: { workspaceId },
      });
    } catch (error) {
      throw createTauriCommandError('list_project_permission_grants', error, { workspaceId });
    }
  }

  async removeProjectGrant(workspaceId: string, grant: Pick<PermissionGrant, 'action' | 'resource'>): Promise<boolean> {
    const request = { workspaceId, action: grant.action, resource: grant.resource };
    try {
      return await api.invoke<boolean>('remove_project_permission_grant', { request });
    } catch (error) {
      throw createTauriCommandError('remove_project_permission_grant', error, request);
    }
  }

  async clearProjectGrants(workspaceId: string): Promise<number> {
    try {
      return await api.invoke<number>('clear_project_permission_grants', {
        request: { workspaceId },
      });
    } catch (error) {
      throw createTauriCommandError('clear_project_permission_grants', error, { workspaceId });
    }
  }

  async listProjectAudit(workspaceId: string, page = 0, pageSize = 50): Promise<PermissionAuditPage> {
    const request = { workspaceId, page, pageSize };
    try {
      return await api.invoke<PermissionAuditPage>('list_project_permission_audit', { request });
    } catch (error) {
      throw createTauriCommandError('list_project_permission_audit', error, request);
    }
  }
}

export const permissionAPI = new PermissionAPI();

import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({ api: { invoke: invokeMock } }));

describe('PermissionAPI', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('scopes grant listing to a backend workspace id', async () => {
    invokeMock.mockResolvedValueOnce([]);
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.listProjectGrants('workspace-1');

    expect(invokeMock).toHaveBeenCalledWith('list_project_permission_grants', {
      request: { workspaceId: 'workspace-1' },
    });
  });

  it('removes grants without accepting a frontend project id', async () => {
    invokeMock.mockResolvedValueOnce(true);
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.removeProjectGrant('workspace-1', {
      action: 'edit',
      resource: 'src/main.rs',
    });

    expect(invokeMock).toHaveBeenCalledWith('remove_project_permission_grant', {
      request: {
        workspaceId: 'workspace-1',
        action: 'edit',
        resource: 'src/main.rs',
      },
    });
  });

  it('clears grants using only the backend workspace id', async () => {
    invokeMock.mockResolvedValueOnce(2);
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.clearProjectGrants('workspace-1');

    expect(invokeMock).toHaveBeenCalledWith('clear_project_permission_grants', {
      request: { workspaceId: 'workspace-1' },
    });
  });

  it('passes explicit audit pagination', async () => {
    invokeMock.mockResolvedValueOnce({ records: [], page: 2, pageSize: 25, total: 0 });
    const { permissionAPI } = await import('./PermissionAPI');

    await permissionAPI.listProjectAudit('workspace-1', 2, 25);

    expect(invokeMock).toHaveBeenCalledWith('list_project_permission_audit', {
      request: { workspaceId: 'workspace-1', page: 2, pageSize: 25 },
    });
  });
});

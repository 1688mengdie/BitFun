import { useState, useEffect, useCallback } from 'react';
import { WorkspaceInfo, globalStateAPI } from '../../shared/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useWorkspace');

 
export const useWorkspace = () => {
  const [currentWorkspace, setCurrentWorkspace] = useState<WorkspaceInfo | null>(null);
  const [recentWorkspaces, setRecentWorkspaces] = useState<WorkspaceInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  
  const loadCurrentWorkspace = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const workspace = await globalStateAPI.getCurrentWorkspace();
      setCurrentWorkspace(workspace);
      
      if (workspace) {
        log.debug('Workspace loaded', {
          id: workspace.id,
          name: workspace.name,
          rootPath: workspace.rootPath
        });
      }
    } catch (err) {
      log.error('Failed to load current workspace', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  
  const loadRecentWorkspaces = useCallback(async () => {
    try {
      const workspaces = await globalStateAPI.getRecentWorkspaces();
      setRecentWorkspaces(workspaces);
    } catch (err) {
      log.error('Failed to load recent workspaces', err);
    }
  }, []);

  
  const openWorkspace = useCallback(async (path: string): Promise<WorkspaceInfo> => {
    try {
      setLoading(true);
      setError(null);
      
      log.info('Opening workspace', { path });
      const workspace = await globalStateAPI.openWorkspace(path);
      
      setCurrentWorkspace(workspace);
      
      log.info('Workspace opened', {
        id: workspace.id,
        name: workspace.name,
        rootPath: workspace.rootPath,
        type: workspace.workspaceType
      });
      
      
      await loadRecentWorkspaces();
      
      return workspace;
    } catch (err) {
      log.error('Failed to open workspace', { path, error: err });
      setError(err instanceof Error ? err.message : String(err));
      throw err;
    } finally {
      setLoading(false);
    }
  }, [loadRecentWorkspaces]);

  
  const closeWorkspace = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      
      log.info('Closing workspace');
      await globalStateAPI.closeWorkspace();
      
      setCurrentWorkspace(null);
      
      
      await loadRecentWorkspaces();
    } catch (err) {
      log.error('Failed to close workspace', err);
      setError(err instanceof Error ? err.message : String(err));
      throw err;
    } finally {
      setLoading(false);
    }
  }, [loadRecentWorkspaces]);

  
  const scanWorkspaceInfo = useCallback(async (): Promise<WorkspaceInfo | null> => {
    try {
      setLoading(true);
      setError(null);
      
      
      if (!currentWorkspace?.rootPath) {
        const errorMsg = `No current workspace available for scanning`;
        log.error('Cannot scan workspace', {
          hasWorkspace: !!currentWorkspace,
          workspaceId: currentWorkspace?.id,
          hasRootPath: !!currentWorkspace?.rootPath
        });
        throw new Error(errorMsg);
      }
      
      log.debug('Scanning workspace info', { rootPath: currentWorkspace.rootPath });
      const workspace = await globalStateAPI.scanWorkspaceInfo(currentWorkspace.rootPath);
      
      if (workspace) {
        log.debug('Workspace info updated', {
          statistics: workspace.statistics,
          languages: workspace.languages
        });
        setCurrentWorkspace(workspace);
      }
      
      return workspace;
    } catch (err) {
      log.error('Failed to scan workspace info', err);
      setError(err instanceof Error ? err.message : String(err));
      throw err;
    } finally {
      setLoading(false);
    }
  }, [currentWorkspace]);

  
  useEffect(() => {
    const initializeWorkspace = async () => {
      await Promise.all([
        loadCurrentWorkspace(),
        loadRecentWorkspaces()
      ]);
    };
    
    initializeWorkspace();
  }, [loadCurrentWorkspace, loadRecentWorkspaces]);

  return {
    
    currentWorkspace,
    recentWorkspaces,
    loading,
    error,
    
    
    openWorkspace,
    closeWorkspace,
    scanWorkspaceInfo,
    loadCurrentWorkspace,
    loadRecentWorkspaces,
    
    
    hasWorkspace: !!currentWorkspace,
    workspaceName: currentWorkspace?.name || '',
    workspacePath: currentWorkspace?.rootPath || '',
  };
};

 
export const useCurrentWorkspace = () => {
  const { currentWorkspace, loading, error } = useWorkspace();
  
  return {
    workspace: currentWorkspace,
    loading,
    error,
    hasWorkspace: !!currentWorkspace,
    workspaceName: currentWorkspace?.name || '',
    workspacePath: currentWorkspace?.rootPath || '',
  };
};

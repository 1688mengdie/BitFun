 

import React, { createContext, useContext, useEffect, useState, useCallback, useRef, ReactNode, useMemo } from 'react';
import { workspaceManager, WorkspaceState, WorkspaceEvent } from '../services/business/workspaceManager';
import { WorkspaceInfo } from '../../shared/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceProvider');

interface WorkspaceContextValue extends WorkspaceState {
  activeWorkspace: WorkspaceInfo | null;
  openedWorkspacesList: WorkspaceInfo[];
  openWorkspace: (path: string) => Promise<WorkspaceInfo>;
  closeWorkspace: () => Promise<void>;
  closeWorkspaceById: (workspaceId: string) => Promise<void>;
  switchWorkspace: (workspace: WorkspaceInfo) => Promise<WorkspaceInfo>;
  setActiveWorkspace: (workspaceId: string) => Promise<WorkspaceInfo>;
  scanWorkspaceInfo: () => Promise<WorkspaceInfo | null>;
  refreshRecentWorkspaces: () => Promise<void>;
  hasWorkspace: boolean;
  workspaceName: string;
  workspacePath: string;
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

interface WorkspaceProviderProps {
  children: ReactNode;
}

export const WorkspaceProvider: React.FC<WorkspaceProviderProps> = ({ children }) => {
  const [state, setState] = useState<WorkspaceState>(() => {
    try {
      return workspaceManager.getState();
    } catch (error) {
      log.warn('WorkspaceManager not initialized, using default state', error);
      return {
        currentWorkspace: null,
        openedWorkspaces: new Map(),
        activeWorkspaceId: null,
        lastUsedWorkspaceId: null,
        recentWorkspaces: [],
        loading: false,
        error: null,
      };
    }
  });

  const isInitializedRef = useRef(false);

  useEffect(() => {
    const removeListener = workspaceManager.addEventListener((_event: WorkspaceEvent) => {
      const newState = workspaceManager.getState();

      setState(prevState => {
        const prevOpenedIds = Array.from(prevState.openedWorkspaces.keys()).join('|');
        const nextOpenedIds = Array.from(newState.openedWorkspaces.keys()).join('|');
        const isChanged =
          prevState.currentWorkspace?.id !== newState.currentWorkspace?.id ||
          prevState.activeWorkspaceId !== newState.activeWorkspaceId ||
          prevState.lastUsedWorkspaceId !== newState.lastUsedWorkspaceId ||
          prevState.loading !== newState.loading ||
          prevState.error !== newState.error ||
          prevState.recentWorkspaces.length !== newState.recentWorkspaces.length ||
          prevOpenedIds !== nextOpenedIds;

        return isChanged ? newState : prevState;
      });
    });

    return () => {
      removeListener();
    };
  }, []);

  useEffect(() => {
    const initializeWorkspace = async () => {
      if (isInitializedRef.current) {
        return;
      }

      try {
        isInitializedRef.current = true;
        setState(prev => ({ ...prev, loading: true }));
        await workspaceManager.initialize();
        setState(workspaceManager.getState());
      } catch (error) {
        log.error('Failed to initialize workspace state', error);
        isInitializedRef.current = false;
        setState(prev => ({ ...prev, loading: false, error: String(error) }));
      }
    };

    initializeWorkspace();
  }, []);

  const openWorkspace = useCallback(async (path: string): Promise<WorkspaceInfo> => {
    return await workspaceManager.openWorkspace(path);
  }, []);

  const closeWorkspace = useCallback(async (): Promise<void> => {
    return await workspaceManager.closeWorkspace();
  }, []);

  const closeWorkspaceById = useCallback(async (workspaceId: string): Promise<void> => {
    return await workspaceManager.closeWorkspaceById(workspaceId);
  }, []);

  const switchWorkspace = useCallback(async (workspace: WorkspaceInfo): Promise<WorkspaceInfo> => {
    return await workspaceManager.switchWorkspace(workspace);
  }, []);

  const setActiveWorkspace = useCallback(async (workspaceId: string): Promise<WorkspaceInfo> => {
    return await workspaceManager.setActiveWorkspace(workspaceId);
  }, []);

  const scanWorkspaceInfo = useCallback(async (): Promise<WorkspaceInfo | null> => {
    return await workspaceManager.scanWorkspaceInfo();
  }, []);

  const refreshRecentWorkspaces = useCallback(async (): Promise<void> => {
    return await workspaceManager.refreshRecentWorkspaces();
  }, []);

  const activeWorkspace = state.currentWorkspace;
  const openedWorkspacesList = useMemo(
    () => Array.from(state.openedWorkspaces.values()),
    [state.openedWorkspaces]
  );
  const hasWorkspace = !!activeWorkspace;
  const workspaceName = activeWorkspace?.name || '';
  const workspacePath = activeWorkspace?.rootPath || '';

  const contextValue: WorkspaceContextValue = {
    ...state,
    activeWorkspace,
    openedWorkspacesList,
    openWorkspace,
    closeWorkspace,
    closeWorkspaceById,
    switchWorkspace,
    setActiveWorkspace,
    scanWorkspaceInfo,
    refreshRecentWorkspaces,
    hasWorkspace,
    workspaceName,
    workspacePath,
  };

  return (
    <WorkspaceContext.Provider value={contextValue}>
      {children}
    </WorkspaceContext.Provider>
  );
};

export const useWorkspaceContext = (): WorkspaceContextValue => {
  const context = useContext(WorkspaceContext);

  if (!context) {
    throw new Error('useWorkspaceContext must be used within a WorkspaceProvider');
  }

  return context;
};

export const useCurrentWorkspace = () => {
  const { activeWorkspace, loading, error, hasWorkspace, workspaceName, workspacePath } = useWorkspaceContext();

  return {
    workspace: activeWorkspace,
    loading,
    error,
    hasWorkspace,
    workspaceName,
    workspacePath,
  };
};

export const useWorkspaceEvents = (
  onWorkspaceOpened?: (workspace: WorkspaceInfo) => void,
  onWorkspaceClosed?: (workspaceId: string) => void,
  onWorkspaceSwitched?: (workspace: WorkspaceInfo) => void,
  onWorkspaceUpdated?: (workspace: WorkspaceInfo) => void
) => {
  useEffect(() => {
    const removeListener = workspaceManager.addEventListener((event: WorkspaceEvent) => {
      switch (event.type) {
        case 'workspace:opened':
          onWorkspaceOpened?.(event.workspace);
          break;
        case 'workspace:closed':
          onWorkspaceClosed?.(event.workspaceId);
          break;
        case 'workspace:switched':
          onWorkspaceSwitched?.(event.workspace);
          break;
        case 'workspace:updated':
          onWorkspaceUpdated?.(event.workspace);
          break;
      }
    });

    return removeListener;
  }, [onWorkspaceOpened, onWorkspaceClosed, onWorkspaceSwitched, onWorkspaceUpdated]);
};

export { WorkspaceContext };

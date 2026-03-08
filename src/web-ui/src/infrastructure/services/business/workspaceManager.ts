 

import { WorkspaceInfo, globalStateAPI } from '../../../shared/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceManager');

export type WorkspaceEvent =
  | { type: 'workspace:opened'; workspace: WorkspaceInfo }
  | { type: 'workspace:closed'; workspaceId: string }
  | { type: 'workspace:switched'; workspace: WorkspaceInfo }
  | { type: 'workspace:active-changed'; workspace: WorkspaceInfo | null }
  | { type: 'workspace:updated'; workspace: WorkspaceInfo }
  | { type: 'workspace:loading'; loading: boolean }
  | { type: 'workspace:error'; error: string | null };

export type WorkspaceEventListener = (event: WorkspaceEvent) => void;

export interface WorkspaceState {
  currentWorkspace: WorkspaceInfo | null;
  openedWorkspaces: Map<string, WorkspaceInfo>;
  activeWorkspaceId: string | null;
  lastUsedWorkspaceId: string | null;
  recentWorkspaces: WorkspaceInfo[];
  loading: boolean;
  error: string | null;
}

class WorkspaceManager {
  private static instance: WorkspaceManager | null = null;
  private state: WorkspaceState;
  private listeners: Set<WorkspaceEventListener> = new Set();
  private isInitialized = false;
  private isInitializing = false;

  private constructor() {
    this.state = {
      currentWorkspace: null,
      openedWorkspaces: new Map(),
      activeWorkspaceId: null,
      lastUsedWorkspaceId: null,
      recentWorkspaces: [],
      loading: true,
      error: null,
    };
  }

  public static getInstance(): WorkspaceManager {
    if (!WorkspaceManager.instance) {
      WorkspaceManager.instance = new WorkspaceManager();
    }
    return WorkspaceManager.instance;
  }

  public getState(): WorkspaceState {
    return {
      ...this.state,
      openedWorkspaces: new Map(this.state.openedWorkspaces),
    };
  }

  public addEventListener(listener: WorkspaceEventListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private emit(event: WorkspaceEvent): void {
    log.debug('Emitting event', { type: event.type });
    this.listeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Event listener execution error', { eventType: event.type, error });
      }
    });
  }

  private updateState(updates: Partial<WorkspaceState>, event?: WorkspaceEvent): void {
    this.state = {
      ...this.state,
      ...updates,
      openedWorkspaces: updates.openedWorkspaces
        ? new Map(updates.openedWorkspaces)
        : this.state.openedWorkspaces,
    };

    log.debug('State updated', {
      activeWorkspaceId: this.state.activeWorkspaceId,
      openedWorkspaceCount: this.state.openedWorkspaces.size,
    });

    if (event) {
      this.emit(event);
    }
  }

  private setLoading(loading: boolean): void {
    this.updateState({ loading }, { type: 'workspace:loading', loading });
  }

  private setError(error: string | null): void {
    this.updateState({ error }, { type: 'workspace:error', error });
  }

  private buildOpenedWorkspaceMap(workspaces: WorkspaceInfo[]): Map<string, WorkspaceInfo> {
    return new Map(workspaces.map(workspace => [workspace.id, workspace]));
  }

  private resolveLastUsedWorkspaceId(
    currentWorkspace: WorkspaceInfo | null,
    recentWorkspaces: WorkspaceInfo[],
    openedWorkspaces: Map<string, WorkspaceInfo>
  ): string | null {
    return (
      currentWorkspace?.id ||
      recentWorkspaces[0]?.id ||
      openedWorkspaces.keys().next().value ||
      null
    );
  }

  private updateWorkspaceState(
    currentWorkspace: WorkspaceInfo | null,
    recentWorkspaces: WorkspaceInfo[],
    openedWorkspaces: WorkspaceInfo[],
    loading: boolean,
    error: string | null,
    event?: WorkspaceEvent
  ): void {
    const openedWorkspaceMap = this.buildOpenedWorkspaceMap(openedWorkspaces);
    const resolvedCurrentWorkspace = currentWorkspace
      ? openedWorkspaceMap.get(currentWorkspace.id) ?? currentWorkspace
      : null;

    this.updateState(
      {
        currentWorkspace: resolvedCurrentWorkspace,
        openedWorkspaces: openedWorkspaceMap,
        activeWorkspaceId: resolvedCurrentWorkspace?.id ?? null,
        lastUsedWorkspaceId: this.resolveLastUsedWorkspaceId(
          resolvedCurrentWorkspace,
          recentWorkspaces,
          openedWorkspaceMap
        ),
        recentWorkspaces,
        loading,
        error,
      },
      event
    );
  }

  public async initialize(): Promise<void> {
    if (this.isInitialized || this.isInitializing) {
      return;
    }

    try {
      this.isInitializing = true;
      log.info('Initializing workspace state');

      const initResult = await globalStateAPI.initializeGlobalState();
      log.debug('Backend initialization completed', { result: initResult });

      const [recentWorkspaces, openedWorkspaces, currentWorkspace] = await Promise.all([
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
        globalStateAPI.getCurrentWorkspace(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        currentWorkspace
          ? { type: 'workspace:opened', workspace: currentWorkspace }
          : undefined
      );

      this.emit({ type: 'workspace:loading', loading: false });
      this.isInitialized = true;
      log.info('Workspace state initialization completed', {
        activeWorkspaceId: currentWorkspace?.id ?? null,
        openedWorkspaceCount: openedWorkspaces.length,
      });
    } catch (error) {
      log.error('Failed to initialize workspace state', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateWorkspaceState(null, [], [], false, errorMessage);
      this.emit({ type: 'workspace:error', error: errorMessage });
    } finally {
      this.isInitializing = false;
    }
  }

  public async openWorkspace(path: string): Promise<WorkspaceInfo> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Opening workspace', { path });

      const workspace = await globalStateAPI.openWorkspace(path);
      const [recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        workspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:opened', workspace }
      );

      return workspace;
    } catch (error) {
      log.error('Failed to open workspace', { path, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async closeWorkspace(): Promise<void> {
    if (!this.state.currentWorkspace?.id) {
      return;
    }

    await this.closeWorkspaceById(this.state.currentWorkspace.id);
  }

  public async closeWorkspaceById(workspaceId: string): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);

      log.info('Closing workspace', { workspaceId });

      await globalStateAPI.closeWorkspace(workspaceId);

      const [currentWorkspace, recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getCurrentWorkspace(),
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        currentWorkspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:closed', workspaceId }
      );

      this.emit({ type: 'workspace:active-changed', workspace: currentWorkspace });
    } catch (error) {
      log.error('Failed to close workspace', { workspaceId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async setActiveWorkspace(workspaceId: string): Promise<WorkspaceInfo> {
    try {
      if (this.state.activeWorkspaceId === workspaceId) {
        const currentWorkspace = this.state.currentWorkspace;
        if (!currentWorkspace) {
          throw new Error(`Active workspace not found: ${workspaceId}`);
        }
        return currentWorkspace;
      }

      this.setLoading(true);
      this.setError(null);

      const workspace = await globalStateAPI.setActiveWorkspace(workspaceId);
      const [recentWorkspaces, openedWorkspaces] = await Promise.all([
        globalStateAPI.getRecentWorkspaces(),
        globalStateAPI.getOpenedWorkspaces(),
      ]);

      this.updateWorkspaceState(
        workspace,
        recentWorkspaces,
        openedWorkspaces,
        false,
        null,
        { type: 'workspace:switched', workspace }
      );

      this.emit({ type: 'workspace:active-changed', workspace });
      return workspace;
    } catch (error) {
      log.error('Failed to set active workspace', { workspaceId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.updateState({ loading: false, error: errorMessage }, { type: 'workspace:error', error: errorMessage });
      throw error;
    }
  }

  public async switchWorkspace(workspace: WorkspaceInfo): Promise<WorkspaceInfo> {
    if (this.state.currentWorkspace?.id === workspace.id) {
      return workspace;
    }

    if (this.state.openedWorkspaces.has(workspace.id)) {
      return this.setActiveWorkspace(workspace.id);
    }

    return this.openWorkspace(workspace.rootPath);
  }

  public async scanWorkspaceInfo(): Promise<WorkspaceInfo | null> {
    try {
      if (!this.state.currentWorkspace?.rootPath) {
        throw new Error('No current workspace available for scanning');
      }

      this.setLoading(true);
      this.setError(null);

      const updatedWorkspace = await globalStateAPI.scanWorkspaceInfo(this.state.currentWorkspace.rootPath);

      if (updatedWorkspace) {
        const openedWorkspaces = new Map(this.state.openedWorkspaces);
        openedWorkspaces.set(updatedWorkspace.id, updatedWorkspace);

        const recentWorkspaces = this.state.recentWorkspaces.map(workspace =>
          workspace.id === updatedWorkspace.id ? updatedWorkspace : workspace
        );

        this.updateState(
          {
            currentWorkspace: updatedWorkspace,
            openedWorkspaces,
            recentWorkspaces,
            activeWorkspaceId: updatedWorkspace.id,
            loading: false,
            error: null,
          },
          { type: 'workspace:updated', workspace: updatedWorkspace }
        );
      } else {
        this.setLoading(false);
      }

      return updatedWorkspace;
    } catch (error) {
      log.error('Failed to scan workspace info', { error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      this.setError(errorMessage);
      this.setLoading(false);
      throw error;
    }
  }

  public async refreshRecentWorkspaces(): Promise<void> {
    try {
      const recentWorkspaces = await globalStateAPI.getRecentWorkspaces();
      this.updateState({ recentWorkspaces });
      log.debug('Recent workspaces refreshed', { count: recentWorkspaces.length });
    } catch (error) {
      log.error('Failed to refresh recent workspaces', { error });
    }
  }

  public hasWorkspace(): boolean {
    return !!this.state.currentWorkspace;
  }

  public getWorkspaceName(): string {
    return this.state.currentWorkspace?.name || '';
  }

  public getWorkspacePath(): string {
    return this.state.currentWorkspace?.rootPath || '';
  }
}

export const workspaceManager = WorkspaceManager.getInstance();

export { WorkspaceManager };



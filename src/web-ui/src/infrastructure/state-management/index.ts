 

export * from './types';
export * from './Store';
export * from './GlobalStore';

// React Hooks
import React from 'react';
import { useStore } from './Store';
import { 
  globalStore, 
  GlobalStateActions,
  selectAppState,
  selectWorkspaceState,
  selectChatState,
  selectAgentState,
  selectPluginState,
  selectSidebarCollapsed,
  selectRightPanelCollapsed,
  selectCurrentTheme,
  selectIsLoading,
  selectError,
  selectNotifications,
  selectCurrentWorkspace,
  selectRecentWorkspaces,
  selectWorkspaceLoading,
  selectChatSessions,
  selectCurrentChatSession
} from './GlobalStore';
import type { GlobalState, AppState, WorkspaceState, ChatState, AgentGlobalState, PluginGlobalState } from './types';


export function useGlobalState(): GlobalState {
  return useStore(globalStore);
}


export function useAppState(): AppState {
  return useStore(globalStore, selectAppState);
}

export function useWorkspaceState(): WorkspaceState {
  return useStore(globalStore, selectWorkspaceState);
}

export function useChatState(): ChatState {
  return useStore(globalStore, selectChatState);
}

export function useAgentState(): AgentGlobalState {
  return useStore(globalStore, selectAgentState);
}

export function usePluginState(): PluginGlobalState {
  return useStore(globalStore, selectPluginState);
}


export function useSidebarCollapsed(): boolean {
  return useStore(globalStore, selectSidebarCollapsed);
}

export function useRightPanelCollapsed(): boolean {
  return useStore(globalStore, selectRightPanelCollapsed);
}

export function useCurrentTheme(): string {
  return useStore(globalStore, selectCurrentTheme);
}

export function useIsLoading(): boolean {
  return useStore(globalStore, selectIsLoading);
}

export function useError(): string | null {
  return useStore(globalStore, selectError);
}

export function useNotifications(): import('./types').Notification[] {
  return useStore(globalStore, selectNotifications);
}

export function useCurrentWorkspace(): import('./types').Workspace | null {
  return useStore(globalStore, selectCurrentWorkspace);
}

export function useRecentWorkspaces(): import('./types').Workspace[] {
  return useStore(globalStore, selectRecentWorkspaces);
}

export function useWorkspaceLoading(): boolean {
  return useStore(globalStore, selectWorkspaceLoading);
}

export function useChatSessions(): import('./types').ChatSession[] {
  return useStore(globalStore, selectChatSessions);
}

export function useCurrentChatSession(): import('./types').ChatSession | null {
  return useStore(globalStore, selectCurrentChatSession);
}


export function useGlobalActions() {
  return React.useMemo(() => ({
    // App Actions
    toggleSidebar: GlobalStateActions.toggleSidebar,
    toggleRightPanel: GlobalStateActions.toggleRightPanel,
    setTheme: GlobalStateActions.setTheme,
    setLoading: GlobalStateActions.setLoading,
    setError: GlobalStateActions.setError,
    addNotification: GlobalStateActions.addNotification,
    removeNotification: GlobalStateActions.removeNotification,

    // Workspace Actions
    setCurrentWorkspace: GlobalStateActions.setCurrentWorkspace,
    addRecentWorkspace: GlobalStateActions.addRecentWorkspace,
    setWorkspaceLoading: GlobalStateActions.setWorkspaceLoading,
    setWorkspaceError: GlobalStateActions.setWorkspaceError,

    // Chat Actions
    addChatSession: GlobalStateActions.addChatSession,
    setCurrentChatSession: GlobalStateActions.setCurrentChatSession,
    addChatMessage: GlobalStateActions.addChatMessage,
    setChatInput: GlobalStateActions.setChatInput,
    setChatProcessing: GlobalStateActions.setChatProcessing,

    // Agent Actions
    setActiveAgent: GlobalStateActions.setActiveAgent,
    removeActiveAgent: GlobalStateActions.removeActiveAgent,

    // Plugin Actions
    setInstalledPlugins: GlobalStateActions.setInstalledPlugins,
    activatePlugin: GlobalStateActions.activatePlugin,
    deactivatePlugin: GlobalStateActions.deactivatePlugin,
  }), []);
}


export function useAppStore() {
  const state = useAppState();
  const actions = useGlobalActions();
  
  return React.useMemo(() => ({
    state,
    toggleSidebar: actions.toggleSidebar,
    toggleRightPanel: actions.toggleRightPanel,
    setTheme: actions.setTheme,
    setLoading: actions.setLoading,
    setError: actions.setError,
    addNotification: actions.addNotification,
    removeNotification: actions.removeNotification,
  }), [state, actions]);
}

export function useWorkspaceStore() {
  const state = useWorkspaceState();
  const actions = useGlobalActions();
  
  return React.useMemo(() => ({
    state,
    setCurrentWorkspace: actions.setCurrentWorkspace,
    addRecentWorkspace: actions.addRecentWorkspace,
    setLoading: actions.setWorkspaceLoading,
    setError: actions.setWorkspaceError,
  }), [state, actions]);
}

export function useChatStore() {
  const state = useChatState();
  const actions = useGlobalActions();
  
  return React.useMemo(() => ({
    state,
    addSession: actions.addChatSession,
    setCurrentSession: actions.setCurrentChatSession,
    addMessage: actions.addChatMessage,
    setInput: actions.setChatInput,
    setProcessing: actions.setChatProcessing,
  }), [state, actions]);
}



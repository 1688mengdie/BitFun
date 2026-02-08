 

import { Store } from './Store';
import { GlobalState, AppState, WorkspaceState, ChatState, AgentGlobalState, PluginGlobalState } from './types';
import { globalEventBus } from '../event-bus/EventBus';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('GlobalState');


const initialGlobalState: GlobalState = {
  app: {
    sidebarCollapsed: false,
    rightPanelCollapsed: true,
    currentTheme: 'dark',
    isLoading: false,
    error: null,
    notifications: [],
    modals: []
  },
  workspace: {
    currentWorkspace: null,
    recentWorkspaces: [],
    loading: false,
    error: null
  },
  chat: {
    sessions: [],
    currentSessionId: null,
    input: '',
    isProcessing: false,
    error: null
  },
  agent: {
    activeAgents: {},
    availableAgents: [],
    currentAgentId: null,
    globalContext: {}
  },
  plugin: {
    installedPlugins: [],
    activePlugins: [],
    availablePlugins: [],
    loading: false,
    error: null
  }
};


export const globalStore = new Store<GlobalState>({
  initialState: initialGlobalState,
  persistence: {
    key: 'bitfun-global-state',
    whitelist: ['app', 'workspace'], 
    storage: localStorage
  },
  devTools: process.env.NODE_ENV === 'development'
});


export const selectAppState = (state: GlobalState) => state.app;
export const selectWorkspaceState = (state: GlobalState) => state.workspace;
export const selectChatState = (state: GlobalState) => state.chat;
export const selectAgentState = (state: GlobalState) => state.agent;
export const selectPluginState = (state: GlobalState) => state.plugin;


export const selectSidebarCollapsed = (state: GlobalState) => state.app.sidebarCollapsed;
export const selectRightPanelCollapsed = (state: GlobalState) => state.app.rightPanelCollapsed;
export const selectCurrentTheme = (state: GlobalState) => state.app.currentTheme;
export const selectIsLoading = (state: GlobalState) => state.app.isLoading;
export const selectError = (state: GlobalState) => state.app.error;
export const selectNotifications = (state: GlobalState) => state.app.notifications;

export const selectCurrentWorkspace = (state: GlobalState) => state.workspace.currentWorkspace;
export const selectRecentWorkspaces = (state: GlobalState) => state.workspace.recentWorkspaces;
export const selectWorkspaceLoading = (state: GlobalState) => state.workspace.loading;

export const selectChatSessions = (state: GlobalState) => state.chat.sessions;
export const selectCurrentChatSession = (state: GlobalState) => {
  const { sessions, currentSessionId } = state.chat;
  return sessions.find(session => session.id === currentSessionId) || null;
};


export class GlobalStateActions {
  // App Actions
  static toggleSidebar(): void {
    globalStore.setState(state => ({
      ...state,
      app: {
        ...state.app,
        sidebarCollapsed: !state.app.sidebarCollapsed
      }
    }));
  }

  static toggleRightPanel(): void {
    globalStore.setState(state => ({
      ...state,
      app: {
        ...state.app,
        rightPanelCollapsed: !state.app.rightPanelCollapsed
      }
    }));
  }

  static setTheme(theme: string): void {
    globalStore.setState(state => ({
      ...state,
      app: {
        ...state.app,
        currentTheme: theme
      }
    }));
  }

  static setLoading(loading: boolean, message?: string): void {
    globalStore.setState(state => ({
      ...state,
      app: {
        ...state.app,
        isLoading: loading,
        loadingMessage: message
      }
    }));
  }

  static setError(error: string | null): void {
    globalStore.setState(state => ({
      ...state,
      app: {
        ...state.app,
        error
      }
    }));
  }

  static addNotification(notification: Omit<import('./types').Notification, 'id' | 'timestamp'>): void {
    const newNotification: import('./types').Notification = {
      ...notification,
      id: `notification-${Date.now()}-${Math.random()}`,
      timestamp: new Date()
    };

    globalStore.setState(state => ({
      ...state,
      app: {
        ...state.app,
        notifications: [...state.app.notifications, newNotification]
      }
    }));

    
    if (newNotification.duration) {
      setTimeout(() => {
        GlobalStateActions.removeNotification(newNotification.id);
      }, newNotification.duration);
    }
  }

  static removeNotification(id: string): void {
    globalStore.setState(state => ({
      ...state,
      app: {
        ...state.app,
        notifications: state.app.notifications.filter(n => n.id !== id)
      }
    }));
  }

  // Workspace Actions
  static setCurrentWorkspace(workspace: import('./types').Workspace | null): void {
    globalStore.setState(state => ({
      ...state,
      workspace: {
        ...state.workspace,
        currentWorkspace: workspace,
        error: null
      }
    }));

    
    if (workspace) {
      GlobalStateActions.addRecentWorkspace(workspace);
    }
  }

  static addRecentWorkspace(workspace: import('./types').Workspace): void {
    globalStore.setState(state => {
      const recentWorkspaces = state.workspace.recentWorkspaces.filter(w => w.id !== workspace.id);
      recentWorkspaces.unshift({ ...workspace, lastOpened: new Date() });
      
      return {
        ...state,
        workspace: {
          ...state.workspace,
          recentWorkspaces: recentWorkspaces.slice(0, 10) 
        }
      };
    });
  }

  static setWorkspaceLoading(loading: boolean): void {
    globalStore.setState(state => ({
      ...state,
      workspace: {
        ...state.workspace,
        loading
      }
    }));
  }

  static setWorkspaceError(error: string | null): void {
    globalStore.setState(state => ({
      ...state,
      workspace: {
        ...state.workspace,
        error,
        loading: false
      }
    }));
  }

  // Chat Actions
  static addChatSession(session: import('./types').ChatSession): void {
    globalStore.setState(state => ({
      ...state,
      chat: {
        ...state.chat,
        sessions: [...state.chat.sessions, session],
        currentSessionId: session.id
      }
    }));
  }

  static setCurrentChatSession(sessionId: string | null): void {
    globalStore.setState(state => ({
      ...state,
      chat: {
        ...state.chat,
        currentSessionId: sessionId
      }
    }));
  }

  static addChatMessage(sessionId: string, message: import('./types').ChatMessage): void {
    globalStore.setState(state => ({
      ...state,
      chat: {
        ...state.chat,
        sessions: state.chat.sessions.map(session =>
          session.id === sessionId
            ? {
                ...session,
                messages: [...session.messages, message],
                updatedAt: new Date()
              }
            : session
        )
      }
    }));
  }

  static setChatInput(input: string): void {
    globalStore.setState(state => ({
      ...state,
      chat: {
        ...state.chat,
        input
      }
    }));
  }

  static setChatProcessing(processing: boolean): void {
    globalStore.setState(state => ({
      ...state,
      chat: {
        ...state.chat,
        isProcessing: processing
      }
    }));
  }

  // Agent Actions
  static setActiveAgent(agentId: string, agentState: import('./types').AgentInstanceState): void {
    globalStore.setState(state => ({
      ...state,
      agent: {
        ...state.agent,
        activeAgents: {
          ...state.agent.activeAgents,
          [agentId]: agentState
        },
        currentAgentId: agentId
      }
    }));
  }

  static removeActiveAgent(agentId: string): void {
    globalStore.setState(state => {
      const { [agentId]: removed, ...rest } = state.agent.activeAgents;
      return {
        ...state,
        agent: {
          ...state.agent,
          activeAgents: rest,
          currentAgentId: state.agent.currentAgentId === agentId ? null : state.agent.currentAgentId
        }
      };
    });
  }

  // Plugin Actions
  static setInstalledPlugins(plugins: import('./types').PluginInfo[]): void {
    globalStore.setState(state => ({
      ...state,
      plugin: {
        ...state.plugin,
        installedPlugins: plugins
      }
    }));
  }

  static activatePlugin(pluginId: string): void {
    globalStore.setState(state => ({
      ...state,
      plugin: {
        ...state.plugin,
        activePlugins: [...state.plugin.activePlugins, pluginId],
        installedPlugins: state.plugin.installedPlugins.map(plugin =>
          plugin.id === pluginId ? { ...plugin, isActive: true } : plugin
        )
      }
    }));
  }

  static deactivatePlugin(pluginId: string): void {
    globalStore.setState(state => ({
      ...state,
      plugin: {
        ...state.plugin,
        activePlugins: state.plugin.activePlugins.filter(id => id !== pluginId),
        installedPlugins: state.plugin.installedPlugins.map(plugin =>
          plugin.id === pluginId ? { ...plugin, isActive: false } : plugin
        )
      }
    }));
  }
}


export function initializeGlobalState(): void {
  log.info('Initializing global state');
  
  
  globalEventBus.on('workspace:opened', (workspace) => {
    GlobalStateActions.setCurrentWorkspace(workspace);
  });

  globalEventBus.on('workspace:closed', () => {
    GlobalStateActions.setCurrentWorkspace(null);
  });

  globalEventBus.on('plugin:activated', ({ pluginId }) => {
    GlobalStateActions.activatePlugin(pluginId);
  });

  globalEventBus.on('plugin:deactivated', ({ pluginId }) => {
    GlobalStateActions.deactivatePlugin(pluginId);
  });

  globalEventBus.on('ui:notification', (notification) => {
    GlobalStateActions.addNotification(notification);
  });

  log.info('Global state initialized');
}


export function destroyGlobalState(): void {
  globalStore.destroy();
}


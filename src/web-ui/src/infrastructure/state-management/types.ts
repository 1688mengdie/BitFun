 


export type StateListener<T = any> = (state: T, prevState: T) => void;


export type StateUnsubscriber = () => void;


export type StateSelector<TState, TResult> = (state: TState) => TResult;


export type StateAction<TState, TPayload = any> = (state: TState, payload: TPayload) => TState;


export type StateMiddleware<TState> = (
  action: string,
  payload: any,
  state: TState,
  next: (newState: TState) => void
) => void;


export interface StoreConfig<TState> {
   
  initialState: TState;
   
  validator?: (state: TState) => boolean;
   
  persistence?: PersistenceConfig<TState>;
   
  devTools?: boolean;
   
  middleware?: StateMiddleware<TState>[];
}


export interface PersistenceConfig<TState> {
   
  key: string;
   
  storage?: Storage;
   
  serialize?: (state: TState) => string;
   
  deserialize?: (data: string) => TState;
   
  whitelist?: (keyof TState)[];
   
  blacklist?: (keyof TState)[];
}


export interface IStore<TState> {
   
  getState(): TState;
  
   
  setState(state: Partial<TState> | ((prevState: TState) => TState)): void;
  
   
  subscribe(listener: StateListener<TState>): StateUnsubscriber;
  
   
  select<TResult>(selector: StateSelector<TState, TResult>): TResult;
  
   
  destroy(): void;
}


export interface GlobalState {
  
  app: AppState;
  
  workspace: WorkspaceState;
  
  chat: ChatState;
  
  agent: AgentGlobalState;
  
  plugin: PluginGlobalState;
}


export interface AppState {
  
  sidebarCollapsed: boolean;
  rightPanelCollapsed: boolean;
  currentTheme: string;
  
  
  isLoading: boolean;
  loadingMessage?: string;
  
  
  error: string | null;
  
  
  notifications: Notification[];
  
  
  modals: ModalState[];
}


export interface WorkspaceState {
  currentWorkspace: Workspace | null;
  recentWorkspaces: Workspace[];
  loading: boolean;
  error: string | null;
}

export interface Workspace {
  id: string;
  name: string;
  rootPath: string;
  lastOpened: Date;
  settings?: Record<string, any>;
}


export interface ChatState {
  sessions: ChatSession[];
  currentSessionId: string | null;
  input: string;
  isProcessing: boolean;
  error: string | null;
}

export interface ChatSession {
  id: string;
  name: string;
  messages: ChatMessage[];
  createdAt: Date;
  updatedAt: Date;
  agentType?: string;
}

export interface ChatMessage {
  id: string;
  type: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: Date;
  metadata?: Record<string, any>;
}


export interface AgentGlobalState {
  activeAgents: Record<string, AgentInstanceState>;
  availableAgents: AgentMetadata[];
  currentAgentId: string | null;
  globalContext: Record<string, any>;
}

export interface AgentInstanceState {
  id: string;
  type: string;
  isActive: boolean;
  isProcessing: boolean;
  context: Record<string, any>;
  history: any[];
  error: string | null;
}

export interface AgentMetadata {
  id: string;
  name: string;
  description: string;
  version: string;
  capabilities: string[];
}


export interface PluginGlobalState {
  installedPlugins: PluginInfo[];
  activePlugins: string[];
  availablePlugins: PluginInfo[];
  loading: boolean;
  error: string | null;
}

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  isInstalled: boolean;
  isActive: boolean;
  permissions: string[];
}


export interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message: string;
  timestamp: Date;
  duration?: number;
  actions?: NotificationAction[];
}

export interface NotificationAction {
  label: string;
  action: () => void;
  type?: 'primary' | 'secondary';
}


export interface ModalState {
  id: string;
  type: string;
  isOpen: boolean;
  data?: any;
  options?: ModalOptions;
}

export interface ModalOptions {
  closable?: boolean;
  maskClosable?: boolean;
  keyboard?: boolean;
  size?: 'small' | 'medium' | 'large';
}


export interface StateChangeEvent<TState> {
  type: 'state:change';
  payload: {
    state: TState;
    prevState: TState;
    changedKeys: string[];
  };
}


export interface StateValidationError extends Error {
  code: 'INVALID_STATE';
  field: string;
  value: any;
  message: string;
}


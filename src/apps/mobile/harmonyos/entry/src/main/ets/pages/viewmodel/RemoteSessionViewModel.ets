import { RemoteSession, SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { RemoteChatCommandController } from '../../services/RemoteChatCommandController';
import { RemoteModelController } from '../../services/RemoteModelController';
import { RemoteSessionController } from '../../services/RemoteSessionController';
import { RemoteFileDownloadController } from '../../services/RemoteFileDownloadController';
import { RemotePageState } from '../state/RemotePageState';

export interface RemoteSessionViewModelHooks {
  readonly remoteAvailable: () => boolean;
  readonly isConnected: () => boolean;
  readonly isBusy: () => boolean;
  readonly onBusy: (busy: boolean) => void;
  readonly onRouteChat: (sessionId: string) => void;
  readonly onRouteHome: () => void;
  readonly onStopPolling: () => void;
  readonly onStartPolling: () => void;
  readonly onResetTimeline: (sessionId: string) => void;
  readonly onClearRemoteFiles: () => void;
  readonly onKnownStateReset: () => void;
  readonly onLoadModelCatalog: (sessionId: string) => Promise<void>;
  readonly onLoadActiveMessages: () => Promise<void>;
  readonly onRefreshSessions: () => Promise<void>;
  readonly onSelectWorkspace: (path: string) => Promise<void>;
}

/** Owns remote session commands and their page lifecycle effects. */
export class RemoteSessionViewModel {
  private readonly pageState: RemotePageState;
  private readonly sessions: RemoteSessionController;
  private readonly chat: RemoteChatCommandController;
  private readonly models: RemoteModelController;
  private readonly files: RemoteFileDownloadController;
  private readonly hooks: RemoteSessionViewModelHooks;

  constructor(
    pageState: RemotePageState,
    sessions: RemoteSessionController,
    chat: RemoteChatCommandController,
    models: RemoteModelController,
    files: RemoteFileDownloadController,
    hooks: RemoteSessionViewModelHooks
  ) {
    this.pageState = pageState;
    this.sessions = sessions;
    this.chat = chat;
    this.models = models;
    this.files = files;
    this.hooks = hooks;
  }

  async refreshSessions(): Promise<void> {
    await this.sessions.refresh(
      this.pageState.sessionQuery,
      this.pageState.sessionFilter,
      this.hooks.remoteAvailable(),
      this.hooks.isConnected()
    );
  }

  async loadMoreSessions(): Promise<void> {
    await this.sessions.loadMore(
      this.pageState.sessionQuery,
      this.pageState.sessionFilter,
      this.hooks.isBusy(),
      this.hooks.remoteAvailable()
    );
  }

  async setFilter(filter: string): Promise<void> {
    if (this.pageState.sessionFilter === filter) {
      return;
    }
    this.pageState.setFilter(filter);
    await this.refreshSessions();
  }

  async createSession(
    agentType: string,
    instruction: string = '',
    onRouteChat: (sessionId: string) => void = this.hooks.onRouteChat,
    modelId: string = ''
  ): Promise<void> {
    if (this.hooks.isBusy() || !this.hooks.remoteAvailable()) {
      return;
    }
    const previousSession: SessionSummary = this.copySession(this.pageState.activeSession);
    this.beginSessionCreation(onRouteChat);
    try {
      const created = await this.performCreateSession(agentType, instruction, onRouteChat, modelId);
      if (!created) {
        this.restoreAfterFailedCreation(previousSession, onRouteChat);
      }
    } finally {
      this.pageState.setConversationLoading(false);
      this.pageState.setPendingSessionId('');
    }
  }

  private async performCreateSession(
    agentType: string,
    instruction: string,
    onRouteChat: (sessionId: string) => void,
    modelId: string
  ): Promise<SessionSummary | undefined> {
    return await this.sessions.create(
      agentType,
      this.hooks.isBusy(),
      this.hooks.remoteAvailable(),
      async (session: SessionSummary): Promise<void> => {
        this.pageState.clearComposer();
        this.pageState.setHasMoreMessages(false);
        this.hooks.onKnownStateReset();
        this.hooks.onResetTimeline(session.sessionId);
        onRouteChat(session.sessionId);
        await this.hooks.onLoadModelCatalog(session.sessionId);
        await this.refreshSessions();
        await this.hooks.onLoadActiveMessages();
        this.hooks.onStartPolling();
      },
      instruction,
      modelId
    );
  }

  async createSessionInWorkspace(
    path: string,
    currentPath: string,
    instruction: string = '',
    agentType: string = 'code',
    onRouteChat: (sessionId: string) => void = this.hooks.onRouteChat,
    modelId: string = ''
  ): Promise<void> {
    if (this.hooks.isBusy() || !this.hooks.remoteAvailable()) {
      return;
    }
    const previousSession: SessionSummary = this.copySession(this.pageState.activeSession);
    const previousWorkspacePath = currentPath;
    this.beginSessionCreation(onRouteChat);
    let created: SessionSummary | undefined;
    try {
      if (path.length > 0 && path !== currentPath) {
        await this.hooks.onSelectWorkspace(path);
        // Workspace selection clears page discovery state. Creation still owns
        // the detail pane until the create command resolves.
        this.pageState.setConversationLoading(true);
      }
      if (path.length === 0 || path === currentPath) {
        created = await this.performCreateSession(agentType, instruction, onRouteChat, modelId);
      } else if (this.pageState.workspacePath === path) {
        created = await this.performCreateSession(agentType, instruction, onRouteChat, modelId);
      }
      if (!created) {
        if (this.pageState.workspacePath === previousWorkspacePath) {
          this.restoreAfterFailedCreation(previousSession, onRouteChat);
        } else {
          this.pageState.clearActiveSession();
          this.hooks.onResetTimeline('');
          this.hooks.onKnownStateReset();
          this.hooks.onRouteHome();
        }
      }
    } finally {
      this.pageState.setConversationLoading(false);
      this.pageState.setPendingSessionId('');
    }
  }

  private beginSessionCreation(onRouteChat: (sessionId: string) => void): void {
    this.hooks.onStopPolling();
    this.pageState.setPendingSessionId('');
    this.pageState.setConversationLoading(true);
    onRouteChat('');
  }

  private restoreAfterFailedCreation(
    previousSession: SessionSummary,
    onRouteChat: (sessionId: string) => void
  ): void {
    if (previousSession.sessionId.length === 0) {
      this.hooks.onRouteHome();
      return;
    }
    this.pageState.setActiveSession(previousSession);
    onRouteChat(previousSession.sessionId);
    this.hooks.onStartPolling();
  }

  private copySession(session: SessionSummary): SessionSummary {
    return {
      sessionId: session.sessionId,
      title: session.title,
      workspacePath: session.workspacePath,
      agentType: session.agentType,
      initialTurnId: session.initialTurnId
    };
  }

  async openSession(
    item: RemoteSession,
    currentWorkspacePath: string,
    onRouteChat: (sessionId: string) => void = this.hooks.onRouteChat
  ): Promise<void> {
    const isBusy = this.hooks.isBusy();
    const remoteAvailable = this.hooks.remoteAvailable();
    if (isBusy || item.id.length === 0 || !remoteAvailable) {
      return;
    }
    this.pageState.setPendingSessionId(item.id);
    this.pageState.setConversationLoading(true);
    onRouteChat(item.id);
    try {
      await this.sessions.open(
        item,
        item.workspacePath || currentWorkspacePath,
        false,
        true,
        async (session: SessionSummary): Promise<void> => {
          this.hooks.onStopPolling();
          this.hooks.onResetTimeline(item.id);
          this.hooks.onKnownStateReset();
          this.pageState.setHasMoreMessages(false);
          this.files.clear();
          this.pageState.clearComposer();
          await this.hooks.onLoadModelCatalog(item.id);
          await this.hooks.onLoadActiveMessages();
          if (this.pageState.activeSession.sessionId === session.sessionId) {
            this.hooks.onStartPolling();
          }
        }
      );
    } finally {
      this.pageState.setConversationLoading(false);
      this.pageState.setPendingSessionId('');
    }
  }

  async deleteSession(item: RemoteSession, currentWorkspacePath: string): Promise<void> {
    await this.sessions.delete(
      item,
      this.pageState.activeSession.sessionId,
      this.hooks.isBusy(),
      this.hooks.remoteAvailable(),
      this.pageState.sessionQuery,
      this.pageState.sessionFilter,
      async (): Promise<void> => {
        this.hooks.onStopPolling();
        this.hooks.onResetTimeline('');
        this.pageState.setHasMoreMessages(false);
        this.pageState.clearComposer();
        this.models.clearCatalog();
        this.hooks.onKnownStateReset();
        this.pageState.setActiveSession({
          sessionId: '',
          title: '',
          workspacePath: currentWorkspacePath,
          agentType: 'code'
        });
        this.pageState.clearActiveSession();
        this.hooks.onRouteHome();
      }
    );
  }

  async loadActiveMessages(isChatRoute: (sessionId: string) => boolean): Promise<void> {
    const sessionId = this.pageState.activeSession.sessionId || '';
    await this.chat.loadMessages(sessionId, isChatRoute);
  }

  async loadOlderMessages(knownPollVersion: number): Promise<void> {
    await this.chat.loadOlderMessages(
      this.pageState.activeSession.sessionId || '',
      knownPollVersion,
      this.pageState.hasMoreMessages,
      this.hooks.isBusy()
    );
  }

  async loadModelCatalog(sessionId: string, isChatRoute: (sessionId: string) => boolean): Promise<void> {
    await this.models.loadCatalog(sessionId, this.hooks.remoteAvailable(), isChatRoute);
  }

  async selectModel(modelId: string): Promise<void> {
    await this.models.selectModel(
      modelId,
      this.pageState.activeSession.sessionId || '',
      this.hooks.isBusy(),
      this.hooks.remoteAvailable()
    );
  }
}

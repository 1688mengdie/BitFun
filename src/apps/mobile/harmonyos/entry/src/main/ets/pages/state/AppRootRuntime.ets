import {
  ChatMessage,
  RemoteModelCatalog,
  RemoteImageContext,
  RemoteQuestionAnswerPayload,
  RemoteSession,
  SelectedImageAttachment,
  SessionSummary,
  WorkspaceInfo
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ClipboardService } from '../../services/ClipboardService';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ChatTimelineState } from '../../services/ChatTimelineStore';
import { ConnectionErrorPolicy } from '../../services/ConnectionErrorPolicy';
import { ImagePickerService } from '../../services/ImagePickerService';
import {
  GeneralChatConfigSnapshot,
  GeneralChatConfigStore,
  GeneralChatConfigUpdate
} from '../../services/general-chat/GeneralChatConfigStore';
import { GeneralChatBootstrapController } from '../../services/general-chat/GeneralChatBootstrapController';
import { GeneralChatCommandController } from '../../services/general-chat/GeneralChatCommandController';
import { GeneralChatController } from '../../services/general-chat/GeneralChatController';
import { GeneralChatDraftController } from '../../services/general-chat/GeneralChatDraftController';
import { GeneralChatDraftLifecycleController } from '../../services/general-chat/GeneralChatDraftLifecycleController';
import { GeneralChatStreamLifecycleController } from '../../services/general-chat/GeneralChatStreamLifecycleController';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../../services/general-chat/GeneralChatServiceState';
import {
  GeneralChatSendResult,
  GeneralChatStreamCallbacks
} from '../../services/general-chat/GeneralChatPort';
import { MobileIdentityStore } from '../../services/MobileIdentityStore';
import { AppRootRouteState } from '../../services/AppRootRouteState';
import { RemoteActivityLifecycleController } from '../../services/RemoteActivityLifecycleController';
import { RemoteChatCommandController } from '../../services/RemoteChatCommandController';
import {
  RemoteChatPollingCursor,
  RemoteChatPollingLifecycleController,
  RemoteChatPollingSnapshot
} from '../../services/RemoteChatPollingLifecycleController';
import { RemoteFileDownloadController } from '../../services/RemoteFileDownloadController';
import { RemoteLogger } from '../../services/RemoteLogger';
import { RemoteModelController } from '../../services/RemoteModelController';
import { RemotePairingPolicy } from '../../services/RemotePairingPolicy';
import { RemoteSessionController } from '../../services/RemoteSessionController';
import { RemoteSessionManager } from '../../services/RemoteSessionManager';
import { RemoteWorkspaceRepository } from '../../services/RemoteWorkspaceRepository';
import { RemoteWorkspaceCoordinator } from '../../services/RemoteWorkspaceCoordinator';
import { RemoteConnectionCoordinator } from '../../services/RemoteConnectionCoordinator';
import { RemoteToolActionController } from '../../services/RemoteToolActionController';
import { AsyncLifecycleGate } from '../../services/AsyncLifecycleGate';
import { QrScanService } from '../../services/QrScanService';
import { RemoteUiState } from '../../services/RemoteUiState';
import {
  VoiceInputLifecycleController,
  VoiceInputRouteSnapshot
} from '../../services/VoiceInputLifecycleController';
import { VoiceInputService } from '../../services/VoiceInputService';
import { ConversationIntent } from '../components/ConversationIntent';
import { AppRootHostPort } from '../host/AppRootHostAdapter';
import {
  AppRootPresentation,
  AppRootPresentationActions,
  ConnectPresentationActions,
  RemoteHomePresentationActions,
  SettingsPresentationActions,
  SidebarPresentationActions
} from '../components/AppRootPresentation';
import {
  AppNavigationBackAction,
  AppRoute,
  AppRouteContract
} from '../navigation/AppRouteContract';
import { AppShellState } from './AppShellState';
import { AppShellViewModel } from './AppShellViewModel';
import {
  RemoteActivityViewModel,
  RemoteActivityViewModelHooks
} from './RemoteActivityViewModel';
import {
  RemoteConnectionViewModel
} from './RemoteConnectionViewModel';
import {
  ConversationIntentDispatcher,
  ConversationIntentDispatcherHooks
} from './ConversationIntentDispatcher';
import { GeneralChatPageState } from './GeneralChatPageState';
import { RemotePageState } from './RemotePageState';
import { ConversationViewModel } from './ConversationViewModel';
import {
  RemoteWorkspaceViewModel,
  RemoteWorkspaceViewModelHooks
} from './RemoteWorkspaceViewModel';
import {
  RemoteSessionViewModel,
  RemoteSessionViewModelHooks
} from './RemoteSessionViewModel';
import {
  GeneralChatConversationViewModel,
  GeneralChatConversationViewModelHooks
} from './GeneralChatConversationViewModel';

enum ConnectionState {
  Idle = 'idle',
  Parsing = 'parsing',
  Pairing = 'pairing',
  Connected = 'connected',
  Reconnecting = 'reconnecting',
  Failed = 'failed',
  Disconnected = 'disconnected'
}

const GENERAL_CHAT_HOME_DRAFT_ID: string = 'new-chat';
const GENERAL_CHAT_DRAFT_SAVE_DELAY_MS: number = 250;

export class AppRootRuntime {
  readonly host: AppRootHostPort;

  constructor(host: AppRootHostPort) {
    this.host = host;
  }

  readonly sessionManager: RemoteSessionManager = new RemoteSessionManager();
  readonly workspaceRepository: RemoteWorkspaceRepository =
    new RemoteWorkspaceRepository(this.sessionManager);
  readonly workspaceCoordinator: RemoteWorkspaceCoordinator =
    new RemoteWorkspaceCoordinator(this.workspaceRepository);
  readonly remoteResumeGate: AsyncLifecycleGate = new AsyncLifecycleGate();
  readonly remoteConnectionGate: AsyncLifecycleGate = new AsyncLifecycleGate();
  readonly identityStore: MobileIdentityStore = new MobileIdentityStore();
  readonly clipboardService: ClipboardService = new ClipboardService();
  readonly qrScanService: QrScanService = new QrScanService();
  readonly imagePickerService: ImagePickerService = new ImagePickerService();
  readonly remotePairingPolicy: RemotePairingPolicy = new RemotePairingPolicy();
  readonly remoteConnectionCoordinator: RemoteConnectionCoordinator =
    new RemoteConnectionCoordinator(
      this.sessionManager,
      this.identityStore,
      this.remotePairingPolicy,
      this.remoteConnectionGate
    );
  readonly generalChatConfigStore: GeneralChatConfigStore = new GeneralChatConfigStore();
  readonly generalChatController: GeneralChatController =
    GeneralChatController.createDefault(this.generalChatConfigStore);
  readonly generalChatDraftController: GeneralChatDraftController =
    new GeneralChatDraftController(
      this.generalChatController,
      GENERAL_CHAT_DRAFT_SAVE_DELAY_MS,
      (err: Error) => {
        RemoteLogger.warn(`general chat draft operation failed: ${ConnectionErrorPolicy.errorText(err)}`);
      }
    );
  readonly generalChatDraftLifecycleController: GeneralChatDraftLifecycleController =
    new GeneralChatDraftLifecycleController(
      this.generalChatDraftController,
      GENERAL_CHAT_HOME_DRAFT_ID,
      (): string => this.visibleGeneralChatDraftId()
    );
  readonly chatTimelineStore: ConversationViewModel = new ConversationViewModel();
  readonly generalChatCommandController: GeneralChatCommandController =
    new GeneralChatCommandController(
      this.generalChatController,
      {
        onSessions: (sessions: RemoteSession[]) => {
          this.generalChatPageState.setSessions(sessions);
        },
        onSessionPrepared: (sessionId: string) => {
          this.resetGeneralChatTimeline(sessionId);
          this.remoteModelController.clearCatalog();
        },
        onActiveSession: (session: SessionSummary) => {
          this.generalChatPageState.setActiveSession(session);
        },
        onMessagesLoaded: (messages: ChatMessage[]) => {
          this.chatTimelineStore.setPersistedMessages(messages);
          this.syncGeneralChatTimelineFromStore();
        },
        onClearComposer: () => {
          this.generalChatPageState.clearComposer();
        },
        onChatInput: (text: string) => {
          this.generalChatPageState.setChatInput(text);
        },
        onStatusText: (statusText: string) => {
          this.generalChatPageState.setStatus(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.generalChatPageState.setBusy(isBusy);
        },
        onToast: (statusText: string) => {
          this.showHomeToast(statusText);
        }
      }
    );
  readonly generalChatBootstrapController: GeneralChatBootstrapController =
    new GeneralChatBootstrapController(
      this.generalChatConfigStore,
      this.generalChatCommandController,
      this.generalChatDraftLifecycleController,
      {
        onConfigRestored: (snapshot: GeneralChatConfigSnapshot) => {
          this.applyGeneralChatConfig(snapshot);
        },
        onHomeDraftRestored: (text: string) => {
          this.generalChatPageState.setChatInput(text);
        },
        onStatusText: (statusText: string) => {
          this.generalChatPageState.setStatus(statusText);
        }
      }
    );
  readonly voiceInputService: VoiceInputService = new VoiceInputService();
  readonly remoteActivityLifecycleController: RemoteActivityLifecycleController =
    new RemoteActivityLifecycleController(() => {
      this.checkConnectionHealth();
    });
  readonly remoteActivityViewModel: RemoteActivityViewModel =
    new RemoteActivityViewModel(
      this.remoteActivityLifecycleController,
      this.remoteConnectionCoordinator,
      this.remoteResumeGate,
      new RemoteActivityViewModelHooks(
        (): boolean => this.connectionState === ConnectionState.Connected,
        (): boolean => this.isBusy,
        (): boolean => this.hasRemoteBindingForResume(),
        (): boolean => this.isRoute(AppRoute.RemoteChat),
        (): SessionSummary => this.activeSession,
        (state: string): void => this.setRemoteConnectionState(state as ConnectionState),
        (status: string): void => this.setRemoteStatusText(status),
        (): void => this.stopHeartbeat(),
        (): void => this.startPolling(),
        (): void => this.stopPolling(),
        async (): Promise<void> => {
          await this.pollActiveSession();
        },
        async (): Promise<void> => {
          await this.connect(true);
        },
        async (session: SessionSummary): Promise<void> => {
          this.remotePageState.setActiveSession(session);
          await this.loadActiveMessages();
        }
      )
    );
  isSyncingAfterTurn: boolean = false;
  knownPollVersion: number = 0;
  knownModelCatalogVersion: number = 0;
  knownRemoteMessageCount: number = 0;
  readonly generalChatStreamLifecycleController: GeneralChatStreamLifecycleController =
    new GeneralChatStreamLifecycleController();
  readonly generalChatPageState: GeneralChatPageState = new GeneralChatPageState();
  readonly remotePageState: RemotePageState = new RemotePageState();
  readonly remoteWorkspaceViewModel: RemoteWorkspaceViewModel =
    new RemoteWorkspaceViewModel(
      this.remotePageState,
      this.workspaceCoordinator,
      new RemoteWorkspaceViewModelHooks(
        (): boolean => this.ensureRemoteAvailable(),
        (): boolean => this.isBusy,
        (isBusy: boolean): void => {
          this.setRemoteBusy(isBusy);
        },
        (statusText: string): void => {
          this.setRemoteStatusText(statusText);
        },
        (workspace: WorkspaceInfo): void => {
          this.applyWorkspace(workspace);
          this.remoteSessionController.clearSessions();
        },
        (sessions: RemoteSession[]): void => {
          this.applyDiscoveredWorkspaceSessions(sessions);
        },
        async (): Promise<void> => {
          await this.refreshSessions();
        },
        (error: Object): void => {
          this.failRemoteConnection(error);
        }
      )
    );
  remoteWorkspaceSessions: RemoteSession[] = [];
  readonly appShellViewModel: AppShellViewModel = new AppShellViewModel();
  readonly appShellState: AppShellState = this.appShellViewModel.state;
  readonly voiceInputLifecycleController: VoiceInputLifecycleController =
    new VoiceInputLifecycleController(
      this.voiceInputService,
      {
        currentInputText: (): string => this.visibleChatInput(),
        currentStatusText: (): string => this.visibleStatusText(),
        onInputText: (routeId: string, text: string) => {
          this.setChatInputForRoute(routeId as AppRoute, text);
        },
        onListening: (routeId: string, isListening: boolean) => {
          this.setVoiceListeningForRoute(routeId as AppRoute, isListening);
        },
        onStatusText: (statusText: string) => {
          this.setVisibleStatusText(statusText);
        },
        onError: (message: string) => {
          this.showVoiceInputError(message);
        }
      }
    );
  readonly remoteSessionController: RemoteSessionController =
    new RemoteSessionController(
      this.sessionManager,
      8,
      {
        onSessions: (sessions: RemoteSession[], hasMore: boolean) => {
          const extras = this.remoteWorkspaceSessions.filter((item: RemoteSession) => {
            return item.workspacePath !== this.workspacePath;
          });
          this.remotePageState.setSessions(this.mergeSessions(sessions, extras), hasMore);
        },
        onActiveSession: (session: SessionSummary) => {
          this.remotePageState.setActiveSession(session);
        },
        onStatusText: (statusText: string) => {
          this.setRemoteStatusText(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.setRemoteBusy(isBusy);
        },
        onLoading: (isLoading: boolean) => {
          this.remotePageState.setLoading(isLoading);
        },
        onSessionError: (errorText: string) => {
          this.remotePageState.setError(errorText);
        },
        onReconnecting: () => {
          this.setRemoteConnectionState(ConnectionState.Reconnecting);
        },
        onConnected: () => {
          this.setRemoteConnectionState(ConnectionState.Connected);
        },
        onConnectionFailed: (err: Object) => {
          this.failRemoteConnection(err);
        },
        onStartHeartbeat: () => {
          this.startHeartbeat();
        }
      }
    );
  readonly remoteChatCommandController: RemoteChatCommandController =
    new RemoteChatCommandController(
      this.sessionManager,
      {
        onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => {
          this.chatTimelineStore.setPersistedMessages(messages);
          this.remotePageState.setHasMoreMessages(hasMoreMessages);
          this.syncChatTimelineFromStore();
        },
        onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => {
          this.knownRemoteMessageCount = knownMessageCount;
          this.updateChatPollingCursor(pollVersion, knownMessageCount);
        },
        onSendSucceeded: (turnId: string, pendingActiveId: string) => {
          if (turnId.length > 0) {
            this.chatTimelineStore.setLocalActiveTurn(turnId);
            this.syncChatTimelineFromStore();
          } else if (pendingActiveId.length > 0) {
            this.chatTimelineStore.clearPendingActiveTurn(pendingActiveId);
            this.syncChatTimelineFromStore();
          }
          this.nudgeChatPolling();
        },
        onSendFailed: (
          rawText: string,
          images: SelectedImageAttachment[],
          localMessageId: string,
          pendingActiveId: string
        ) => {
          this.remotePageState.setChatInput(rawText);
          this.remotePageState.setSelectedImages(images);
          this.chatTimelineStore.markOptimisticMessageFailed(localMessageId);
          if (pendingActiveId.length > 0) {
            this.chatTimelineStore.clearPendingActiveTurn(pendingActiveId);
          }
          this.syncChatTimelineFromStore();
        },
        onActiveSession: (session: SessionSummary) => {
          this.remotePageState.setActiveSession(session);
        },
        onSessionTitleChanged: (sessionId: string, title: string) => {
          this.remoteSessionController.updateSessionTitle(sessionId, title);
        },
        onStatusText: (statusText: string) => {
          this.setRemoteStatusText(statusText);
        },
        onBusy: (isBusy: boolean) => {
          this.setRemoteBusy(isBusy);
        },
        onPollRequested: () => {
          this.pollActiveSession();
        }
      }
    );
  readonly remoteFileDownloadController: RemoteFileDownloadController =
    new RemoteFileDownloadController(
      this.sessionManager,
      (downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string) => {
        this.remotePageState.setDownloadStatus(downloadingFilePath, downloadedFilePath, fileDownloadStatus);
      },
      () => {
        this.remotePageState.clearDownloadingFilePath();
      },
      (statusText: string) => {
        this.setRemoteStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.setRemoteBusy(isBusy);
      }
    );
  readonly remoteToolActionController: RemoteToolActionController =
    new RemoteToolActionController(
      this.sessionManager,
      (statusText: string) => {
        this.setRemoteStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.setRemoteBusy(isBusy);
      },
      () => {
        this.pollActiveSession();
      }
    );
  readonly remoteChatPollingLifecycleController: RemoteChatPollingLifecycleController =
    new RemoteChatPollingLifecycleController(
      this.sessionManager,
      {
        canPoll: (sessionId: string) => {
          return this.activeSession.sessionId === sessionId &&
            this.isRoute(AppRoute.RemoteChat) &&
            this.ensureRemoteAvailable();
        },
        onSnapshot: (snapshot: RemoteChatPollingSnapshot) => {
          this.applyChatSessionSnapshot(snapshot);
        },
        onError: (error: Object) => {
          this.setRemoteStatusText(ConnectionErrorPolicy.errorText(error));
        }
      }
    );
  readonly remoteModelController: RemoteModelController =
    new RemoteModelController(
      this.sessionManager,
      this.identityStore,
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.knownModelCatalogVersion = knownModelCatalogVersion;
        this.remoteChatPollingLifecycleController.updateKnownModelCatalogVersion(knownModelCatalogVersion);
        this.remotePageState.setModelCatalog(modelCatalog, selectedModelId);
      },
      (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
        this.knownModelCatalogVersion = knownModelCatalogVersion;
        this.remoteChatPollingLifecycleController.updateKnownModelCatalogVersion(knownModelCatalogVersion);
        this.chatTimelineStore.setModelCatalog(modelCatalog, selectedModelId);
        this.remotePageState.setModelCatalog(modelCatalog, selectedModelId);
      },
      (statusText: string) => {
        this.setRemoteStatusText(statusText);
      },
      (isBusy: boolean) => {
        this.setRemoteBusy(isBusy);
      }
    );
  readonly remoteSessionViewModel: RemoteSessionViewModel =
    new RemoteSessionViewModel(
      this.remotePageState,
      this.remoteSessionController,
      this.remoteChatCommandController,
      this.remoteModelController,
      this.remoteFileDownloadController,
      new RemoteSessionViewModelHooks(
        (): boolean => this.ensureRemoteAvailable(),
        (): boolean => this.connectionState === ConnectionState.Connected,
        (): boolean => this.isBusy,
        (busy: boolean): void => this.setRemoteBusy(busy),
        (sessionId: string): void => this.pushRoute(AppRoute.RemoteChat, sessionId),
        (): void => this.replaceRoute(AppRoute.RemoteHome),
        (): void => this.stopPolling(),
        (): void => this.startPolling(),
        (sessionId: string): void => this.resetChatTimeline(sessionId),
        (): void => this.remoteFileDownloadController.clear(),
        (): void => {
          this.knownPollVersion = 0;
          this.knownModelCatalogVersion = 0;
          this.knownRemoteMessageCount = 0;
        },
        async (sessionId: string): Promise<void> => {
          await this.remoteModelController.loadCatalog(
            sessionId,
            this.ensureRemoteAvailable(),
            (activeSessionId: string): boolean => {
              return this.activeSession.sessionId === activeSessionId && this.isRoute(AppRoute.RemoteChat);
            }
          );
        },
        async (): Promise<void> => {
          const sessionId = this.activeSession.sessionId || '';
          await this.remoteChatCommandController.loadMessages(
            sessionId,
            (activeSessionId: string): boolean => {
              return this.activeSession.sessionId === activeSessionId && this.isRoute(AppRoute.RemoteChat);
            }
          );
        },
        async (): Promise<void> => {
          await this.remoteSessionController.refresh(
            this.remotePageState.sessionQuery,
            this.remotePageState.sessionFilter,
            this.ensureRemoteAvailable(),
            this.connectionState === ConnectionState.Connected
          );
        },
        async (path: string): Promise<void> => {
          await this.selectWorkspace(path);
        }
      )
    );
  readonly generalChatConversationViewModel: GeneralChatConversationViewModel =
    new GeneralChatConversationViewModel(
      this.generalChatPageState,
      this.generalChatCommandController,
      this.generalChatDraftLifecycleController,
      this.generalChatStreamLifecycleController,
      this.chatTimelineStore,
      new GeneralChatConversationViewModelHooks(
        (sessionId: string): boolean => this.generalChatPageState.activeSession.sessionId === sessionId &&
          this.isGeneralChatVisible(),
        (): string => this.currentActiveTurnId(),
        (): string => this.latestUserMessageText(),
        (): void => this.syncGeneralChatTimelineFromStore(),
        (): void => this.generalChatCommandController.refreshSessions()
      )
    );
  readonly remoteConnectionViewModel: RemoteConnectionViewModel =
    new RemoteConnectionViewModel(
      this.remotePageState,
      this.identityStore,
      this.remotePairingPolicy,
      this.remoteConnectionCoordinator,
      this.remoteSessionController,
      this.remoteModelController,
      this.remoteFileDownloadController,
      this.clipboardService,
      this.qrScanService,
      (sessionId: string): void => this.resetChatTimeline(sessionId),
      (): void => {
        this.knownPollVersion = 0;
        this.knownModelCatalogVersion = 0;
        this.knownRemoteMessageCount = 0;
      },
      (): void => this.startHeartbeat(),
      (): void => this.stopHeartbeat(),
      (): void => this.stopPolling(),
      async (): Promise<void> => {
        await this.loadRecentWorkspacesInBackground();
      },
      (route: AppRoute): void => this.replaceRoute(route),
      (): void => this.appShellState.setConnectSheetVisible(false),
      (): void => this.appShellState.setConnectSheetVisible(true)
    );
  readonly conversationIntentDispatcher: ConversationIntentDispatcher =
    new ConversationIntentDispatcher(new ConversationIntentDispatcherHooks(
      (): void => this.openAppSidebar(),
      (): void => this.closeActiveChat(),
      (): void => { this.createSession('code'); },
      (): void => this.prepareNewGeneralChat(),
      (): RemoteSession => this.activeGeneralChatAsRemoteSession(),
      (): string => this.generalChatPageState.activeSession.sessionId,
      (): boolean => this.generalChatPageState.isBusy,
      (sessionId: string): boolean => this.generalChatPageState.pinnedSessionId() === sessionId,
      async (session: RemoteSession, pinned: boolean, busy: boolean): Promise<void> => {
        await this.generalChatCommandController.pinSession(session, pinned, busy);
      },
      async (session: RemoteSession): Promise<void> => { await this.archiveHomeSession(session, true); },
      async (session: RemoteSession): Promise<void> => {
        await this.deleteHomeSession(session);
        this.prepareNewGeneralChat();
      },
      (text: string): void => this.showHomeToast(text),
      (): number => this.activeGeneralUploadedFileCount(),
      async (): Promise<void> => { await this.stopActiveChatTask(); },
      async (): Promise<void> => { await this.loadOlderMessages(); },
      async (id: string, input?: Object): Promise<void> => { await this.approveTool(id, input); },
      async (id: string): Promise<void> => { await this.rejectTool(id); },
      async (id: string): Promise<void> => { await this.cancelTool(id); },
      async (id: string, answers: RemoteQuestionAnswerPayload): Promise<void> => {
        await this.answerQuestion(id, answers);
      },
      async (title: string): Promise<void> => { await this.renameVisibleSession(title); },
      async (text: string): Promise<void> => { await this.copyMessage(text); },
      async (text: string): Promise<void> => { await this.retryVisibleMessage(text); },
      async (id: string): Promise<void> => { await this.selectModel(id); },
      async (): Promise<void> => { await this.pickImages(); },
      (id: string): void => this.removeSelectedImage(id),
      (path: string): void => this.downloadVisibleFile(path),
      async (): Promise<void> => { await this.sendVisibleChatMessage(); },
      async (): Promise<void> => { await this.toggleVoiceInput(); },
      (route: AppRoute, value: string): void => this.onVisibleChatInputChange(route, value)
    ));
  readonly presentationActions: AppRootPresentationActions = new AppRootPresentationActions(
    (route: AppRoute): boolean => this.handleNavigationBack(route),
    (route: AppRoute, intent: ConversationIntent): void => this.handleConversationIntent(route, intent),
    (): void => this.closeAppSidebar(),
    new RemoteHomePresentationActions(
      (): void => this.openAppSidebar(), (): void => this.enterCodeEntry(), (): void => this.openAddConnection(),
      (): void => this.openRemoteControlSettings(), (): void => { this.refreshSessions(); },
      (): void => { this.showRecentWorkspaces(); }, (): void => { this.showAssistants(); },
      (path: string): void => { this.selectWorkspace(path); }, (path: string): void => { this.selectAssistant(path); },
      (): void => this.remotePageState.setWorkspacePickerVisible(false),
      (): void => this.remotePageState.setAssistantPickerVisible(false),
      (query: string): void => this.remotePageState.setQuery(query), (): void => { this.refreshSessions(); },
      (): void => { this.loadMoreSessions(); }, (): void => { this.reconnect(); },
      (): void => { this.disconnect(false); }, (): void => { this.disconnect(true); },
      (agentType: string): void => { this.createSession(agentType); }, (): void => { this.createSession('Claw'); },
      (path: string): void => { this.createSessionInWorkspace(path); },
      (session: RemoteSession): void => this.openHomeSession(session),
      (session: RemoteSession): void => { this.deleteHomeSession(session); }
    ),
    new SidebarPresentationActions(
      (): void => this.closeAppSidebar(),
      (): void => { this.closeAppSidebar(); this.prepareNewGeneralChat(); },
      (): void => { this.closeAppSidebar(); this.enterCodeEntry(); },
      (): void => { this.closeAppSidebar(); this.appShellState.openSettings('general'); },
      (session: RemoteSession): void => { this.closeAppSidebar(); this.openHomeSession(session); },
      (session: RemoteSession, archived: boolean): void => { this.archiveHomeSession(session, archived); },
      (session: RemoteSession): void => { this.exportHomeSession(session); },
      (session: RemoteSession): void => { this.deleteHomeSession(session); }
    ),
    new SettingsPresentationActions(
      (): void => this.appShellState.setSettingsVisible(false),
      (): void => this.openAddConnectionFromSettings(), (): void => { this.disconnect(false); },
      (): void => { this.reconnect(); },
      async (url: string, key: string, model: string, clear: boolean): Promise<string> =>
        this.saveGeneralChatConfig(url, key, model, clear)
    ),
    new ConnectPresentationActions(
      (): void => this.appShellState.setConnectSheetVisible(false),
      (password?: string): void => { this.connect(false, password || ''); },
      (): void => { this.appShellState.setConnectSheetVisible(false); this.disconnect(true); },
      (url: string): void => { this.setRemoteUrl(url); this.applyRemotePairingProjection(url); },
      (user: string): void => this.setRemoteUserId(user),
      (url: string): boolean => this.handleDetectedRemoteUrl(url),
      (visible: boolean): void => this.setRemoteUrlInputVisible(visible),
      (): void => { this.pasteRemoteUrl(); }, (): void => { this.scanRemoteUrl(); }
    ),
    (): string => this.generalChatHomeStatusText()
  );
  readonly navigationStack: NavPathStack = this.appShellViewModel.navigationStack;


  get remoteUrl(): string { return this.remotePageState.remoteUrl; }
  get userId(): string { return this.remotePageState.userId; }
  get authenticatedUserId(): string { return this.remotePageState.authenticatedUserId; }
  get statusText(): string { return this.remotePageState.statusText; }
  get connectionState(): ConnectionState { return this.remotePageState.connectionState as ConnectionState; }
  get connectionFailureKind(): string { return this.remotePageState.connectionFailureKind; }
  get isBusy(): boolean { return this.remotePageState.isBusy; }
  get showRemoteUrlInput(): boolean { return this.remotePageState.showRemoteUrlInput; }
  get workspaceName(): string { return this.remotePageState.workspaceName; }
  get workspacePath(): string { return this.remotePageState.workspacePath; }
  get workspaceBranch(): string { return this.remotePageState.workspaceBranch; }
  get workspaceKind(): string { return this.remotePageState.workspaceKind; }
  get assistantId(): string { return this.remotePageState.assistantId; }
  get desktopName(): string { return this.remotePageState.desktopName; }
  get desktopId(): string { return this.remotePageState.desktopId; }
  get activeSession(): SessionSummary { return this.remotePageState.activeSession; }
  get messages(): ChatMessage[] { return this.remotePageState.persistedMessages; }
  get pendingMessages(): ChatMessage[] { return this.remotePageState.optimisticMessages; }
  get activeTurnMessage(): ChatMessage { return this.remotePageState.activeTurnMessage; }
  get timelineItems(): ChatTimelineItem[] { return this.remotePageState.timelineItems; }
  get hasMoreMessages(): boolean { return this.remotePageState.hasMoreMessages; }

  async aboutToAppear(): Promise<void> {
    this.syncRemotePageSummary();
    await this.generalChatBootstrapController.restore(this.host.context());
    await this.restoreIdentity();
  }

  onPageShow(): void {
    RemoteLogger.info(`page show state=${this.connectionState} route=${this.currentRoute()}`);
    this.resumeRemoteActivity();
  }

  onPageHide(): void {
    RemoteLogger.info(`page hide state=${this.connectionState} route=${this.currentRoute()}`);
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.setRemoteBusy(false);
    this.persistVisibleGeneralChatDraft();
    this.stopGeneralChatStream(true, 'failed');
  }

  aboutToDisappear(): void {
    this.remoteActivityViewModel.invalidate();
    this.remoteConnectionCoordinator.invalidate();
    this.setRemoteBusy(false);
    this.persistVisibleGeneralChatDraft();
    this.stopGeneralChatStream(true, 'failed');
    this.generalChatDraftLifecycleController.cancel();
    this.remoteFileDownloadController.cancel();
    this.voiceInputLifecycleController.cancel(`${this.currentRoute()}`, () => {
      this.setAllVoiceListening(false);
    });
  }

  currentRoute(): AppRoute {
    return this.appShellViewModel.currentRoute();
  }

  isGeneralComposerRoute(route: AppRoute): boolean {
    return AppRootRouteState.isGeneralComposerRoute(route);
  }

  visibleChatInput(): string {
    return AppRootRouteState.chatInput(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  visibleSelectedImages(): SelectedImageAttachment[] {
    return AppRootRouteState.selectedImages(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  visibleVoiceListening(): boolean {
    return AppRootRouteState.voiceListening(this.currentRoute(), this.generalChatPageState, this.remotePageState);
  }

  setChatInputForRoute(route: AppRoute, value: string): void {
    AppRootRouteState.setChatInput(route, value, this.generalChatPageState, this.remotePageState);
  }

  setSelectedImagesForRoute(route: AppRoute, images: SelectedImageAttachment[]): void {
    AppRootRouteState.setSelectedImages(route, images, this.generalChatPageState, this.remotePageState);
  }

  addSelectedImagesForRoute(route: AppRoute, images: SelectedImageAttachment[]): void {
    AppRootRouteState.addSelectedImages(route, images, this.generalChatPageState, this.remotePageState);
  }

  removeSelectedImageForRoute(route: AppRoute, imageId: string): void {
    AppRootRouteState.removeSelectedImage(route, imageId, this.generalChatPageState, this.remotePageState);
  }

  clearComposerForRoute(route: AppRoute): void {
    AppRootRouteState.clearComposer(route, this.generalChatPageState, this.remotePageState);
  }

  setVoiceListeningForRoute(route: AppRoute, isVoiceListening: boolean): void {
    AppRootRouteState.setVoiceListening(
      route,
      isVoiceListening,
      this.generalChatPageState,
      this.remotePageState
    );
  }

  setAllVoiceListening(isVoiceListening: boolean): void {
    this.generalChatPageState.setVoiceListening(isVoiceListening);
    this.remotePageState.setVoiceListening(isVoiceListening);
  }

  voiceInputSnapshot(route: AppRoute = this.currentRoute()): VoiceInputRouteSnapshot {
    return AppRootRouteState.snapshot(
      route,
      this.visibleChatBusy(),
      this.generalChatPageState,
      this.remotePageState
    );
  }

  isRoute(route: AppRoute): boolean {
    return this.appShellViewModel.isRoute(route);
  }

  isGeneralChatVisible(): boolean {
    return this.appShellViewModel.isGeneralChatVisible();
  }

  pushRoute(route: AppRoute, sessionId: string = ''): void {
    this.appShellViewModel.pushRoute(route, sessionId);
  }

  replaceRoute(route: AppRoute, sessionId: string = ''): void {
    this.appShellViewModel.replaceRoute(route, sessionId);
  }

  popRoute(fallback: AppRoute): void {
    this.appShellViewModel.popRoute(fallback);
  }

  handleNavigationBack(route: AppRoute): boolean {
    const action = this.appShellViewModel.backAction(route);
    if (action === AppNavigationBackAction.CloseSidebar) {
      this.closeAppSidebar();
      return true;
    }
    if (action === AppNavigationBackAction.CloseActiveChat) {
      this.closeActiveChat();
      return true;
    }
    if (action === AppNavigationBackAction.PopRemoteHome) {
      this.popRoute(AppRoute.ChatHome);
      return true;
    }
    return false;
  }

  handleConversationIntent(route: AppRoute, intent: ConversationIntent): void {
    this.conversationIntentDispatcher.dispatch(route, intent);
  }


  async saveGeneralChatConfig(
    apiUrl: string,
    apiKey: string,
    modelName: string,
    clearApiKey: boolean
  ): Promise<string> {
    const update: GeneralChatConfigUpdate = {
      apiUrl,
      apiKey,
      modelName,
      clearApiKey
    };
    try {
      const snapshot = await this.generalChatConfigStore.save(update);
      this.applyGeneralChatConfig(snapshot);
      return '';
    } catch (err) {
      return ConnectionErrorPolicy.errorText(err);
    }
  }

  applyGeneralChatConfig(snapshot: GeneralChatConfigSnapshot): void {
    this.generalChatPageState.setConfiguration(
      snapshot.apiUrl,
      snapshot.modelName,
      snapshot.hasApiKey,
      GeneralChatServiceStatus.fromConfiguration(snapshot.apiUrl, snapshot.modelName, snapshot.hasApiKey)
    );
  }

  async restoreIdentity(): Promise<void> {
    await this.remoteConnectionViewModel.restore(this.host.context());
  }

  async connect(autoReconnect: boolean = false, accountPassword: string = ''): Promise<void> {
    await this.remoteConnectionViewModel.connect(autoReconnect, accountPassword);
  }

  async reconnect(): Promise<void> {
    await this.remoteConnectionViewModel.reconnect();
  }

  async disconnect(clearPairing: boolean): Promise<void> {
    await this.remoteConnectionViewModel.disconnect(clearPairing);
  }

  async pasteRemoteUrl(): Promise<void> {
    await this.remoteConnectionViewModel.paste();
  }

  async scanRemoteUrl(): Promise<void> {
    await this.remoteConnectionViewModel.scan(this.host.context());
  }

  handleDetectedRemoteUrl(remoteUrl: string): boolean {
    return this.remoteConnectionViewModel.handleDetectedUrl(remoteUrl);
  }

  applyWorkspace(workspace: WorkspaceInfo): void {
    this.remoteConnectionViewModel.applyWorkspace(workspace);
  }

  applyRemotePairingProjection(remoteUrl: string): void {
    this.remoteConnectionViewModel.projectRemoteUrl(remoteUrl);
  }

  ensureRemoteAvailable(): boolean {
    return this.remoteConnectionViewModel.ensureAvailable();
  }

  setRemoteConnectionState(connectionState: ConnectionState): void {
    this.remotePageState.setConnectionState(connectionState);
  }

  setRemoteUrl(remoteUrl: string): void {
    this.remotePageState.setRemoteUrl(remoteUrl);
  }

  setRemoteUserId(userId: string): void {
    this.remotePageState.setUserId(userId);
  }

  setRemoteAuthenticatedUserId(authenticatedUserId: string): void {
    this.remotePageState.setAuthenticatedUserId(authenticatedUserId);
  }

  setRemoteStatusText(statusText: string): void {
    this.remotePageState.setStatusText(statusText);
  }

  setRemoteConnectionFailureKind(connectionFailureKind: string): void {
    this.remotePageState.setConnectionFailureKind(connectionFailureKind);
  }

  setRemoteBusy(isBusy: boolean): void {
    this.remotePageState.setBusy(isBusy);
  }

  setRemoteUrlInputVisible(visible: boolean): void {
    this.remotePageState.setRemoteUrlInputVisible(visible);
  }

  syncRemotePageSummary(): void {
    if (this.remotePageState.statusText.length === 0) {
      this.remotePageState.setStatusText(RemoteI18n.t('status.waitingConnection'));
    }
    if (this.remotePageState.workspaceName.length === 0) {
      this.remotePageState.setWorkspace(RemoteI18n.t('status.notConnected'), '', '', '', 'normal');
    }
  }

  failRemoteConnection(err: Object): void {
    this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
    this.setRemoteConnectionState(ConnectionState.Failed);
    this.stopHeartbeat();
  }

  async showRecentWorkspaces(): Promise<void> {
    await this.remoteWorkspaceViewModel.toggleRecentWorkspaces();
  }

  async showAssistants(): Promise<void> {
    await this.remoteWorkspaceViewModel.toggleAssistants();
  }

  async selectWorkspace(path: string): Promise<void> {
    await this.remoteWorkspaceViewModel.selectWorkspace(path);
  }

  async selectAssistant(path: string): Promise<void> {
    await this.remoteWorkspaceViewModel.selectAssistant(path);
  }

  async refreshSessions(): Promise<void> {
    await this.remoteSessionViewModel.refreshSessions();
  }

  async loadMoreSessions(): Promise<void> {
    await this.remoteSessionViewModel.loadMoreSessions();
  }

  setSessionFilter(filter: string): void {
    this.remoteSessionViewModel.setFilter(filter);
  }

  visibleChatBusy(): boolean {
    return this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat) ?
      this.generalChatPageState.isBusy : this.remotePageState.isBusy;
  }

  visibleStatusText(): string {
    return this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat) ?
      this.generalChatPageState.statusText : this.remotePageState.statusText;
  }

  setVisibleStatusText(statusText: string): void {
    if (this.isRoute(AppRoute.ChatHome) || this.isRoute(AppRoute.GeneralChat)) {
      this.generalChatPageState.setStatus(statusText);
      return;
    }
    this.setRemoteStatusText(statusText);
  }

  openAppSidebar(): void {
    this.host.animate(230, () => {
      this.appShellState.setSidebarVisible(true);
    });
  }

  closeAppSidebar(): void {
    this.host.animate(210, () => {
      this.appShellState.setSidebarVisible(false);
    });
  }

  enterCodeEntry(): void {
    if (RemoteUiState.canUseRemote(this.connectionState)) {
      this.appShellState.setConnectSheetVisible(false);
      this.pushRoute(AppRoute.RemoteHome);
      return;
    }
    this.appShellState.setConnectSheetVisible(true);
  }

  openAddConnection(): void {
    this.appShellState.setConnectSheetVisible(true);
  }

  openRemoteControlSettings(): void {
    setTimeout(() => {
      this.appShellState.openSettings('remote');
    }, 180);
  }

  openAddConnectionFromSettings(): void {
    this.appShellState.setSettingsVisible(false);
    setTimeout(() => {
      this.openAddConnection();
    }, 220);
  }

  openHomeSession(session: RemoteSession): void {
    if (session.agentType === 'chat') {
      this.openGeneralSession(session);
      return;
    }
    this.openSession(session);
  }

  async deleteHomeSession(session: RemoteSession): Promise<void> {
    if (session.agentType !== 'chat') {
      await this.deleteSession(session);
      return;
    }
    await this.generalChatCommandController.deleteSession(session, this.generalChatPageState.isBusy);
  }

  activeGeneralChatAsRemoteSession(): RemoteSession {
    const active = this.generalChatPageState.activeSession;
    return {
      id: active.sessionId,
      title: active.title,
      agentType: 'chat',
      status: 'ready',
      updatedAt: '',
      createdAt: '',
      messageCount: this.generalChatPageState.timelineItems.length,
      workspacePath: active.workspacePath
    };
  }

  activeGeneralUploadedFileCount(): number {
    let count = 0;
    this.generalChatPageState.timelineItems.forEach((item: ChatTimelineItem) => {
      if (item.message && item.message.images) {
        count += item.message.images.length;
      }
    });
    return count;
  }

  async archiveHomeSession(session: RemoteSession, archived: boolean): Promise<void> {
    await this.generalChatCommandController.archiveSession(session, archived, this.generalChatPageState.isBusy);
  }

  async exportHomeSession(session: RemoteSession): Promise<void> {
    await this.generalChatCommandController.exportSession(
      session,
      this.generalChatPageState.isBusy,
      async (text: string): Promise<void> => {
        await this.clipboardService.writeText(text);
      }
    );
  }

  async openGeneralSession(item: RemoteSession): Promise<void> {
    if (this.generalChatPageState.isBusy) {
      return;
    }
    this.stopPolling();
    this.stopGeneralChatStream(false);
    await this.generalChatCommandController.openSession(
      item,
      this.generalChatPageState.isBusy,
      async (sessionId: string): Promise<string> => {
        return this.generalChatDraftLifecycleController.restore(sessionId);
      },
      (sessionId: string) => {
        this.replaceRoute(AppRoute.ChatHome, sessionId);
      }
    );
  }

  async startGeneralChat(text: string): Promise<void> {
    const trimmed = text.trim();
    if (trimmed.length === 0 || this.generalChatPageState.isBusy) {
      return;
    }
    this.stopPolling();
    this.stopGeneralChatStream(false);
    this.generalChatDraftLifecycleController.cancel();
    const created = await this.generalChatCommandController.createSession(
      trimmed,
      this.generalChatPageState.isBusy,
      async (): Promise<void> => {
        await this.generalChatDraftLifecycleController.clearHomeNow();
      },
      (sessionId: string) => {
        this.replaceRoute(AppRoute.ChatHome, sessionId);
      }
    );
    if (!created) {
      return;
    }
    await this.sendGeneralChatMessage();
  }

  async sendVisibleChatMessage(): Promise<void> {
    if (this.isGeneralChatVisible()) {
      if ((this.generalChatPageState.activeSession.sessionId || '').length === 0) {
        this.startVisibleGeneralChat();
        return;
      }
      await this.sendGeneralChatMessage();
      return;
    }
    await this.sendChatMessage();
  }

  async stopActiveChatTask(): Promise<void> {
    if (this.isGeneralChatVisible()) {
      this.stopGeneralChatStream(true);
      return;
    }
    await this.stopActiveTask();
  }

  closeActiveChat(): void {
    this.stopVoiceInput(false);
    if (this.isRoute(AppRoute.GeneralChat)) {
      this.persistVisibleGeneralChatDraft();
      this.stopGeneralChatStream(true);
      this.popRoute(AppRoute.ChatHome);
      this.restoreGeneralChatDraft(GENERAL_CHAT_HOME_DRAFT_ID);
      return;
    }
    this.stopPolling();
    this.popRoute(AppRoute.RemoteHome);
  }

  async renameVisibleSession(title: string): Promise<void> {
    if (this.isGeneralChatVisible()) {
      await this.generalChatCommandController.renameActiveSession(
        this.generalChatPageState.activeSession,
        title
      );
      return;
    }
    await this.renameActiveSession(title);
  }

  async retryVisibleMessage(text: string): Promise<void> {
    if (this.isGeneralChatVisible()) {
      const sessionId = this.generalChatPageState.activeSession.sessionId || '';
      const prepared = await this.generalChatCommandController.retryMessage(
        sessionId,
        text,
        this.generalChatPageState.isBusy
      );
      if (prepared) {
        await this.sendGeneralChatMessage();
      }
      return;
    }
    this.retryMessage(text);
  }

  downloadVisibleFile(path: string): void {
    if (this.isGeneralChatVisible()) {
      this.generalChatPageState.setStatus(RemoteI18n.t('generalChat.fileDownloadMock'));
      return;
    }
    this.downloadFile(path);
  }

  async createSession(agentType: string): Promise<void> {
    await this.remoteSessionViewModel.createSession(agentType);
  }

  async createSessionInWorkspace(path: string): Promise<void> {
    await this.remoteSessionViewModel.createSessionInWorkspace(path, this.workspacePath);
  }

  applyDiscoveredWorkspaceSessions(all: RemoteSession[]): void {
    this.remoteWorkspaceSessions = all;
    const current = this.remotePageState.sessions;
    const extras = all.filter((item: RemoteSession) => item.workspacePath !== this.workspacePath);
    this.remotePageState.setSessions(this.mergeSessions(current, extras), this.remotePageState.hasMoreSessions);
  }

  async loadRecentWorkspacesInBackground(): Promise<void> {
    await this.remoteWorkspaceViewModel.loadRecentWorkspacesInBackground();
  }

  mergeSessions(primary: RemoteSession[], extras: RemoteSession[]): RemoteSession[] {
    const merged = primary.slice();
    extras.forEach((item: RemoteSession) => {
      if (!merged.some((existing: RemoteSession) => existing.id === item.id)) {
        merged.push(item);
      }
    });
    return merged;
  }

  async openSession(item: RemoteSession): Promise<void> {
    await this.remoteSessionViewModel.openSession(item, this.workspacePath);
  }

  async deleteSession(item: RemoteSession): Promise<void> {
    await this.remoteSessionViewModel.deleteSession(item, this.workspacePath);
  }

  async loadActiveMessages(): Promise<void> {
    await this.remoteSessionViewModel.loadActiveMessages((activeSessionId: string): boolean => {
      return this.activeSession.sessionId === activeSessionId && this.isRoute(AppRoute.RemoteChat);
    });
  }

  async loadModelCatalog(sessionId: string): Promise<void> {
    await this.remoteSessionViewModel.loadModelCatalog(sessionId, (activeSessionId: string): boolean => {
      return this.activeSession.sessionId === activeSessionId && this.isRoute(AppRoute.RemoteChat);
    });
  }

  async selectModel(modelId: string): Promise<void> {
    await this.remoteSessionViewModel.selectModel(modelId);
  }

  async loadOlderMessages(): Promise<void> {
    await this.remoteSessionViewModel.loadOlderMessages(this.knownPollVersion);
  }

  async sendGeneralChatMessage(): Promise<void> {
    await this.generalChatConversationViewModel.sendMessage();
    return;
  }

  stopGeneralChatStream(cancelled: boolean, finalStatus: string = 'cancelled'): void {
    this.generalChatConversationViewModel.stop(cancelled, finalStatus);
    return;
  }

  async sendChatMessage(): Promise<void> {
    if (this.remotePageState.isVoiceListening) {
      await this.stopVoiceInput(false);
    }
    const rawText = this.remotePageState.chatInput.trim();
    const images = this.remotePageState.selectedImages.slice();
    const text = rawText.length > 0 ? rawText : (images.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    const sessionId = this.activeSession.sessionId || '';
    if ((!text && images.length === 0) || !sessionId || this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    this.remotePageState.clearComposer();
    const localMessage = RemoteUiState.localUserMessage(text, images);
    this.chatTimelineStore.appendOptimisticMessage(localMessage);
    const pendingActiveId = this.chatTimelineStore.setPendingActiveTurn(localMessage.id);
    this.syncChatTimelineFromStore();
    RemoteLogger.info(`chat send queued session=${this.shortSessionId(sessionId)} pending=${pendingActiveId}`);
    this.startPolling();
    this.nudgeChatPolling();
    const imageContexts: RemoteImageContext[] = images.length > 0 ? this.imagePickerService.toRemoteContexts(images) : [];
    await this.remoteChatCommandController.sendPreparedMessage(
      sessionId,
      text,
      this.activeSession.agentType,
      rawText,
      images,
      imageContexts,
      localMessage.id,
      pendingActiveId,
      this.isBusy,
      true
    );
  }

  async toggleVoiceInput(): Promise<void> {
    const route = this.currentRoute();
    await this.voiceInputLifecycleController.toggle(this.host.context(), this.voiceInputSnapshot(route));
  }

  async stopVoiceInput(showStatus: boolean): Promise<void> {
    const route = this.currentRoute();
    await this.voiceInputLifecycleController.stop(this.voiceInputSnapshot(route), showStatus);
  }

  showVoiceInputError(message: string): void {
    const text = message.length > 0 ? message : RemoteI18n.t('errors.voiceInputUnavailable');
    this.setVisibleStatusText(text);
    this.host.showToast(text, 2600);
  }

  async pickImages(): Promise<void> {
    if (this.visibleChatBusy()) {
      return;
    }
    const route = this.currentRoute();
    if (this.visibleVoiceListening()) {
      await this.stopVoiceInput(false);
    }
    try {
      this.setVisibleStatusText(RemoteI18n.t('status.pickImage'));
      const picked = await this.imagePickerService.pickImages(3, this.visibleSelectedImages().length);
      if (picked.length === 0) {
        this.setVisibleStatusText(RemoteI18n.t('status.noImageSelected'));
        return;
      }
      this.addSelectedImagesForRoute(route, picked);
      this.setVisibleStatusText(RemoteI18n.f('status.imagesSelected', `${this.visibleSelectedImages().length}`));
    } catch (err) {
      this.setVisibleStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  removeSelectedImage(imageId: string): void {
    this.removeSelectedImageForRoute(this.currentRoute(), imageId);
  }

  startVisibleGeneralChat(): void {
    const rawText = this.generalChatPageState.chatInput.trim();
    const text = rawText.length > 0 ? rawText :
      (this.generalChatPageState.selectedImages.length > 0 ? RemoteI18n.t('chat.analyzeImageDefault') : '');
    if (text.length === 0 || this.generalChatPageState.isBusy) {
      return;
    }
    if (this.generalChatPageState.serviceState === GeneralChatServiceState.Unconfigured) {
      const statusText = GeneralChatServiceStatus.userMessage(this.generalChatPageState.serviceState);
      this.generalChatPageState.setStatus(statusText);
      this.showHomeToast(statusText);
      return;
    }
    this.startGeneralChat(text);
  }

  generalChatHomeStatusText(): string {
    if (this.generalChatPageState.serviceState === GeneralChatServiceState.Ready ||
      this.generalChatPageState.serviceState === GeneralChatServiceState.Sending ||
      this.generalChatPageState.serviceState === GeneralChatServiceState.Streaming) {
      return '';
    }
    return GeneralChatServiceStatus.userMessage(
      this.generalChatPageState.serviceState,
      this.generalChatPageState.statusText
    );
  }

  prepareNewGeneralChat(): void {
    this.stopVoiceInput(false);
    this.stopGeneralChatStream(true);
    this.generalChatDraftLifecycleController.clearHome();
    this.generalChatPageState.clearComposer();
    this.generalChatPageState.clearActiveSession();
    this.resetGeneralChatTimeline('');
    this.replaceRoute(AppRoute.ChatHome);
  }

  onVisibleChatInputChange(route: AppRoute, value: string): void {
    this.setChatInputForRoute(route, value);
    if (!this.isGeneralComposerRoute(route)) {
      return;
    }
    this.generalChatDraftLifecycleController.scheduleVisible(value);
  }

  visibleGeneralChatDraftId(): string {
    if (this.isGeneralChatVisible()) {
      return this.generalChatPageState.activeSession.sessionId || GENERAL_CHAT_HOME_DRAFT_ID;
    }
    return '';
  }

  persistVisibleGeneralChatDraft(): void {
    this.generalChatDraftLifecycleController.persistVisible(this.generalChatPageState.chatInput);
  }

  async restoreGeneralChatDraft(draftId: string): Promise<void> {
    this.generalChatPageState.setChatInput(await this.generalChatDraftLifecycleController.restore(draftId));
  }

  latestUserMessageText(): string {
    if (this.isGeneralChatVisible()) {
      return this.generalChatPageState.latestUserMessageText();
    }
    const candidates = this.messages.concat(this.pendingMessages);
    for (let index = candidates.length - 1; index >= 0; index--) {
      if (candidates[index].role === 'user' && candidates[index].text.trim().length > 0) {
        return candidates[index].text;
      }
    }
    return '';
  }

  showHomeToast(message: string): void {
    if (!this.host.showToast(message, 2600)) {
      this.setVisibleStatusText(message);
    }
  }

  async stopActiveTask(): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    if (!sessionId) {
      return;
    }
    await this.remoteChatCommandController.stopTask(
      sessionId,
      this.activeTurnMessage.id,
      this.currentActiveTurnId(),
      this.ensureRemoteAvailable()
    );
  }

  async renameActiveSession(title: string): Promise<void> {
    const nextTitle = title.trim();
    if (
      !this.activeSession.sessionId ||
        nextTitle.length === 0 ||
        nextTitle === this.activeSession.title ||
        this.isBusy
    ) {
      return;
    }
    await this.remoteChatCommandController.renameActiveSession(
      this.activeSession,
      nextTitle,
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  async copyMessage(text: string): Promise<void> {
    if (text.trim().length === 0) {
      return;
    }
    try {
      await this.clipboardService.writeText(text);
      this.setRemoteStatusText(RemoteI18n.t('status.messageCopied'));
    } catch (err) {
      this.setRemoteStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  async downloadFile(path: string): Promise<void> {
    const sessionId = this.activeSession.sessionId || '';
    await this.remoteFileDownloadController.download(path, sessionId, this.isBusy, this.ensureRemoteAvailable());
  }

  retryMessage(text: string): void {
    if (this.isBusy) {
      return;
    }
    if (!this.ensureRemoteAvailable()) {
      return;
    }
    this.remotePageState.setChatInput(text);
    this.sendChatMessage();
  }

  async approveTool(toolId: string, updatedInput?: Object): Promise<void> {
    await this.remoteToolActionController.approve(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable(),
      updatedInput
    );
  }

  async rejectTool(toolId: string): Promise<void> {
    await this.remoteToolActionController.reject(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  async cancelTool(toolId: string): Promise<void> {
    await this.remoteToolActionController.cancel(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable()
    );
  }

  async answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void> {
    await this.remoteToolActionController.answer(
      toolId,
      this.activeSession.sessionId || '',
      this.isBusy,
      this.ensureRemoteAvailable(),
      answers
    );
  }

  resetChatTimeline(sessionId: string): void {
    this.chatTimelineStore.reset(sessionId);
    this.knownPollVersion = 0;
    this.syncChatTimelineFromStore();
  }

  resetGeneralChatTimeline(sessionId: string): void {
    this.chatTimelineStore.reset(sessionId);
    this.syncGeneralChatTimelineFromStore();
  }

  syncChatTimelineFromStore(): void {
    const state: ChatTimelineState = this.chatTimelineStore.snapshotState();
    const projectedItems: ChatTimelineItem[] = this.projectedTimelineItems();
    this.remotePageState.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      this.hasMoreMessages,
      projectedItems
    );
    this.remotePageState.setModelCatalog(state.modelCatalog, state.selectedModelId);
  }

  syncGeneralChatTimelineFromStore(): void {
    const state: ChatTimelineState = this.chatTimelineStore.snapshotState();
    const projectedItems = this.chatTimelineStore.viewState(false);
    this.generalChatPageState.setTimelineProjection(
      state.persistedMessages,
      state.optimisticMessages,
      state.activeTurn || RemoteUiState.emptyActiveTurn(),
      false,
      projectedItems
    );
    const itemSummary = projectedItems.map((item: ChatTimelineItem) => {
      const message = item.message;
      return `${item.type}:${item.id}:${message ? message.status : ''}:${message ? message.text.length : 0}`;
    }).join(',');
    RemoteLogger.info(`general chat projection revision=${this.generalChatPageState.timelineRevision} persisted=${state.persistedMessages.length} active=${state.activeTurn ? state.activeTurn.id : 'none'} items=${itemSummary}`);
  }

  startPolling(): void {
    this.remoteChatPollingLifecycleController.startActiveSession({
      sessionId: this.activeSession.sessionId || '',
      cursor: this.currentChatPollingCursor(),
      activeTurn: this.activeTurnMessage
    });
  }

  stopPolling(): void {
    this.remoteChatPollingLifecycleController.stop();
  }

  nudgeChatPolling(): void {
    this.remoteChatPollingLifecycleController.nudge();
  }

  async pollActiveSession(): Promise<void> {
    await this.remoteChatPollingLifecycleController.pollNow();
  }

  currentChatPollingCursor(): RemoteChatPollingCursor {
    return {
      pollVersion: this.knownPollVersion,
      knownMessageCount: this.knownRemoteMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    };
  }

  updateChatPollingCursor(pollVersion: number, knownMessageCount: number): void {
    this.knownPollVersion = pollVersion;
    this.knownRemoteMessageCount = knownMessageCount;
    this.remoteChatPollingLifecycleController.updateCursor({
      pollVersion,
      knownMessageCount,
      knownModelCatalogVersion: this.knownModelCatalogVersion
    });
  }

  applyChatSessionSnapshot(snapshot: RemoteChatPollingSnapshot): void {
    if (snapshot.sessionId !== this.activeSession.sessionId || !this.isRoute(AppRoute.RemoteChat)) {
      return;
    }
    this.chatTimelineStore.applySnapshot(snapshot);
    this.syncChatTimelineFromStore();
    this.knownPollVersion = snapshot.cursor.pollVersion;
    this.knownModelCatalogVersion = snapshot.cursor.knownModelCatalogVersion;
    this.knownRemoteMessageCount = snapshot.cursor.knownMessageCount;
    if (snapshot.title.length > 0) {
      this.remotePageState.setActiveSession({
        sessionId: this.activeSession.sessionId,
        title: snapshot.title,
        workspacePath: this.activeSession.workspacePath,
        agentType: this.activeSession.agentType
      });
    }
    if (snapshot.modelCatalog) {
      this.remoteModelController.applyCatalog(snapshot.modelCatalog);
    }
    this.setRemoteStatusText(this.hasRunningActiveTurn()
      ? RemoteI18n.t('status.desktopProcessing')
      : RemoteI18n.t('status.messagesSynced'));
    if (snapshot.shouldSyncAfterTurnEnded) {
      this.syncAfterTurnEnded();
    }
  }

  hasRunningActiveTurn(): boolean {
    return this.activeTurnMessage.id.length > 0 &&
      (this.activeTurnMessage.status || '').toLowerCase() === 'active';
  }

  currentActiveTurnId(): string {
    const activeTurnMessage = this.isGeneralChatVisible() ?
      this.generalChatPageState.activeTurnMessage : this.activeTurnMessage;
    if (activeTurnMessage.turnId && activeTurnMessage.turnId.length > 0) {
      return activeTurnMessage.turnId;
    }
    const activePrefix = 'active-';
    if (activeTurnMessage.id.indexOf(activePrefix) === 0) {
      return activeTurnMessage.id.slice(activePrefix.length);
    }
    return '';
  }

  projectedTimelineItems(): ChatTimelineItem[] {
    return this.chatTimelineStore.viewState(this.hasMoreMessages);
  }

  startHeartbeat(): void {
    this.remoteActivityViewModel.startHeartbeat();
  }

  stopHeartbeat(): void {
    this.remoteActivityViewModel.stopHeartbeat();
  }

  async checkConnectionHealth(): Promise<void> {
    await this.remoteActivityViewModel.checkConnectionHealth();
  }

  resumeRemoteActivity(): void {
    this.remoteActivityViewModel.resume();
  }

  hasRemoteBindingForResume(): boolean {
    return this.remoteUrl.trim().length > 0 &&
      this.userId.trim().length > 0 &&
      this.connectionState !== ConnectionState.Idle &&
      this.connectionState !== ConnectionState.Parsing &&
      this.connectionState !== ConnectionState.Pairing &&
      this.connectionState !== ConnectionState.Disconnected;
  }

  hasRemoteBindingForCodeHome(): boolean {
    return this.remoteUrl.trim().length > 0 &&
      this.userId.trim().length > 0 &&
      (this.connectionState === ConnectionState.Connected ||
        this.connectionState === ConnectionState.Reconnecting ||
        this.connectionState === ConnectionState.Pairing ||
        this.connectionState === ConnectionState.Parsing);
  }

  shortSessionId(sessionId: string): string {
    if (sessionId.length <= 8) {
      return sessionId;
    }
    return sessionId.slice(0, 4) + '...' + sessionId.slice(sessionId.length - 4);
  }

  async syncAfterTurnEnded(): Promise<void> {
    if (this.isSyncingAfterTurn) {
      return;
    }
    this.isSyncingAfterTurn = true;
    try {
      await this.loadActiveMessages();
    } finally {
      this.isSyncingAfterTurn = false;
    }
  }

}

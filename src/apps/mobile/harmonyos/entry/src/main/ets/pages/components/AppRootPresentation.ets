import { RemotePermissionMode, RemoteSession } from '../../model/RemoteModels';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { AppShell } from './AppShell';
import { AppSidebar } from './AppSidebar';
import { ConnectView } from './ConnectView';
import { ConversationIntent } from './ConversationIntent';
import { ConversationViewHost } from './ConversationViewHost';
import { RemoteControlSettingsSheet } from './RemoteControlSettingsSheet';
import { RemoteCreateSessionView } from './RemoteCreateSessionView';
import { RemoteHomeView } from './RemoteHomeView';
import { SettingsSheet } from './SettingsSheet';
import { PAGE_BG } from './Theme';
import { AppNavigationBackAction, AppRoute } from '../navigation/AppRouteContract';
import { AppShellState } from '../state/AppShellState';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemotePageState } from '../state/RemotePageState';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { ConversationViewState } from '../state/ConversationViewState';

export class AppRootPresentationActions {
  readonly onNavigationBack: (route: AppRoute) => boolean;
  readonly onConversationIntent: (route: AppRoute, intent: ConversationIntent) => void;
  readonly onCloseSidebar: () => void;
  readonly onRemoteHome: RemoteHomePresentationActions;
  readonly onRemoteCreate: RemoteCreatePresentationActions;
  readonly onSidebar: SidebarPresentationActions;
  readonly onSettings: SettingsPresentationActions;
  readonly onConnect: ConnectPresentationActions;
  readonly generalStatus: () => string;

  constructor(
    onNavigationBack: (route: AppRoute) => boolean,
    onConversationIntent: (route: AppRoute, intent: ConversationIntent) => void,
    onCloseSidebar: () => void,
    onRemoteHome: RemoteHomePresentationActions,
    onRemoteCreate: RemoteCreatePresentationActions,
    onSidebar: SidebarPresentationActions,
    onSettings: SettingsPresentationActions,
    onConnect: ConnectPresentationActions,
    generalStatus: () => string
  ) {
    this.onNavigationBack = onNavigationBack;
    this.onConversationIntent = onConversationIntent;
    this.onCloseSidebar = onCloseSidebar;
    this.onRemoteHome = onRemoteHome;
    this.onRemoteCreate = onRemoteCreate;
    this.onSidebar = onSidebar;
    this.onSettings = onSettings;
    this.onConnect = onConnect;
    this.generalStatus = generalStatus;
  }
}

export class RemoteCreatePresentationActions {
  readonly back: () => void;
  readonly toggleDevices: () => void;
  readonly toggleWorkspaces: () => void;
  readonly selectDevice: (device: CloudAccountDevice) => void;
  readonly selectWorkspace: (path: string) => void;
  readonly draftChanged: (value: string) => void;
  readonly send: () => void;

  constructor(
    back: () => void,
    toggleDevices: () => void,
    toggleWorkspaces: () => void,
    selectDevice: (device: CloudAccountDevice) => void,
    selectWorkspace: (path: string) => void,
    draftChanged: (value: string) => void,
    send: () => void
  ) {
    this.back = back;
    this.toggleDevices = toggleDevices;
    this.toggleWorkspaces = toggleWorkspaces;
    this.selectDevice = selectDevice;
    this.selectWorkspace = selectWorkspace;
    this.draftChanged = draftChanged;
    this.send = send;
  }
}

export class RemoteHomePresentationActions {
  readonly openSidebar: () => void; readonly connectWorkspace: () => void;
  readonly addConnection: () => void; readonly openSettings: () => void;
  readonly refresh: () => void; readonly showWorkspaces: () => void; readonly showAssistants: () => void;
  readonly selectWorkspace: (path: string) => void; readonly selectAssistant: (path: string) => void;
  readonly cancelWorkspace: () => void; readonly cancelAssistant: () => void;
  readonly queryChanged: (query: string) => void; readonly search: () => void; readonly loadMore: () => void;
  readonly reconnect: () => void; readonly disconnect: () => void; readonly clearPairing: () => void;
  readonly create: (agentType: string) => void; readonly createAssistant: () => void;
  readonly createInWorkspace: (path: string, agentType: string) => void; readonly openSession: (session: RemoteSession) => void;
  readonly deleteSession: (session: RemoteSession) => void;

  constructor(
    openSidebar: () => void, connectWorkspace: () => void, addConnection: () => void, openSettings: () => void,
    refresh: () => void, showWorkspaces: () => void, showAssistants: () => void,
    selectWorkspace: (path: string) => void, selectAssistant: (path: string) => void,
    cancelWorkspace: () => void, cancelAssistant: () => void, queryChanged: (query: string) => void,
    search: () => void, loadMore: () => void, reconnect: () => void, disconnect: () => void,
    clearPairing: () => void, create: (agentType: string) => void, createAssistant: () => void,
    createInWorkspace: (path: string, agentType: string) => void, openSession: (session: RemoteSession) => void,
    deleteSession: (session: RemoteSession) => void
  ) {
    this.openSidebar = openSidebar; this.connectWorkspace = connectWorkspace; this.addConnection = addConnection;
    this.openSettings = openSettings; this.refresh = refresh; this.showWorkspaces = showWorkspaces;
    this.showAssistants = showAssistants; this.selectWorkspace = selectWorkspace; this.selectAssistant = selectAssistant;
    this.cancelWorkspace = cancelWorkspace; this.cancelAssistant = cancelAssistant; this.queryChanged = queryChanged;
    this.search = search; this.loadMore = loadMore; this.reconnect = reconnect; this.disconnect = disconnect;
    this.clearPairing = clearPairing; this.create = create; this.createAssistant = createAssistant;
    this.createInWorkspace = createInWorkspace; this.openSession = openSession; this.deleteSession = deleteSession;
  }
}

export class SidebarPresentationActions {
  readonly close: () => void; readonly newChat: () => void; readonly enterCode: () => void;
  readonly settings: () => void; readonly openAccount: () => void;
  readonly openSession: (session: RemoteSession) => void;
  readonly archive: (session: RemoteSession, archived: boolean) => void;
  readonly exportSession: (session: RemoteSession) => void; readonly deleteSession: (session: RemoteSession) => void;
  constructor(
    close: () => void, newChat: () => void, enterCode: () => void, settings: () => void, openAccount: () => void,
    openSession: (session: RemoteSession) => void, archive: (session: RemoteSession, archived: boolean) => void,
    exportSession: (session: RemoteSession) => void, deleteSession: (session: RemoteSession) => void
  ) {
    this.close = close; this.newChat = newChat; this.enterCode = enterCode; this.settings = settings;
    this.openAccount = openAccount;
    this.openSession = openSession; this.archive = archive; this.exportSession = exportSession; this.deleteSession = deleteSession;
  }
}

export class SettingsPresentationActions {
  readonly close: () => void; readonly addConnection: () => void; readonly disconnect: () => void;
  readonly reconnect: () => void;
  readonly openAccount: () => void;
  readonly cloudLogin: (relayUrl: string, username: string, password: string) => Promise<string>;
  readonly cloudSync: () => Promise<string>;
  readonly cloudLogout: () => Promise<void>;
  readonly cloudListDevices: () => Promise<CloudAccountDevice[]>;
  readonly getPermissionMode: () => Promise<RemotePermissionMode>;
  readonly setPermissionMode: (mode: RemotePermissionMode) => Promise<RemotePermissionMode>;
  readonly saveGeneral: (url: string, key: string, model: string, clear: boolean) => Promise<string>;
  constructor(
    close: () => void, addConnection: () => void, disconnect: () => void, reconnect: () => void,
    openAccount: () => void,
    cloudLogin: (relayUrl: string, username: string, password: string) => Promise<string>,
    cloudSync: () => Promise<string>, cloudLogout: () => Promise<void>,
    cloudListDevices: () => Promise<CloudAccountDevice[]>,
    getPermissionMode: () => Promise<RemotePermissionMode>,
    setPermissionMode: (mode: RemotePermissionMode) => Promise<RemotePermissionMode>,
    saveGeneral: (url: string, key: string, model: string, clear: boolean) => Promise<string>
  ) {
    this.close = close; this.addConnection = addConnection; this.disconnect = disconnect;
    this.reconnect = reconnect; this.openAccount = openAccount; this.cloudLogin = cloudLogin; this.cloudSync = cloudSync;
    this.cloudLogout = cloudLogout; this.cloudListDevices = cloudListDevices;
    this.getPermissionMode = getPermissionMode; this.setPermissionMode = setPermissionMode;
    this.saveGeneral = saveGeneral;
  }
}

export class ConnectPresentationActions {
  readonly back: () => void; readonly connect: (password?: string) => void; readonly clearPairing: () => void;
  readonly urlChanged: (url: string) => void; readonly userChanged: (user: string) => void;
  readonly detected: (url: string) => boolean; readonly inputVisible: (visible: boolean) => void;
  readonly paste: () => void; readonly scan: () => void;
  readonly cloudListDevices: () => Promise<CloudAccountDevice[]>;
  readonly cloudSelectDevice: (device: CloudAccountDevice) => Promise<void>;
  constructor(
    back: () => void, connect: (password?: string) => void, clearPairing: () => void,
    urlChanged: (url: string) => void, userChanged: (user: string) => void,
    detected: (url: string) => boolean, inputVisible: (visible: boolean) => void,
    paste: () => void, scan: () => void, cloudListDevices: () => Promise<CloudAccountDevice[]>,
    cloudSelectDevice: (device: CloudAccountDevice) => Promise<void>
  ) {
    this.back = back; this.connect = connect; this.clearPairing = clearPairing;
    this.urlChanged = urlChanged; this.userChanged = userChanged; this.detected = detected;
    this.inputVisible = inputVisible; this.paste = paste; this.scan = scan;
    this.cloudListDevices = cloudListDevices; this.cloudSelectDevice = cloudSelectDevice;
  }
}

@ComponentV2
export struct AppRootPresentation {
  @Param shellState: AppShellState = new AppShellState();
  @Param navigationStack: NavPathStack = new NavPathStack();
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param remoteCreateState: RemoteCreateSessionState = new RemoteCreateSessionState();
  @Param generalPageState: GeneralChatPageState = new GeneralChatPageState();
  @Param deviceId: string = '';
  @Param currentRoute: AppRoute = AppRoute.ChatHome;
  @Param actions: AppRootPresentationActions = new AppRootPresentationActions(
    () => false, () => {}, () => {},
    new RemoteHomePresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {},
      () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {},
      () => {}, () => {}, () => {}, () => {}, () => {}),
    new RemoteCreatePresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}),
    new SidebarPresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}),
    new SettingsPresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, async (_relayUrl: string, _username: string, _password: string): Promise<string> => '', async (): Promise<string> => '', async (): Promise<void> => {}, async (): Promise<CloudAccountDevice[]> => [], async (): Promise<RemotePermissionMode> => 'ask', async (mode: RemotePermissionMode): Promise<RemotePermissionMode> => mode, async (_url: string, _key: string, _model: string, _clear: boolean): Promise<string> => ''),
    new ConnectPresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, () => false, () => {}, () => {},
      () => {}, async (): Promise<CloudAccountDevice[]> => [], async (_device: CloudAccountDevice): Promise<void> => {}),
    () => ''
  );

  build() {
    Stack() {
      AppShell({ shellState: this.shellState, content: () => { this.NavigationContent(); },
        sidebar: () => { this.SidebarContent(); }, settings: () => { this.SettingsContent(); },
        connect: () => { this.ConnectContent(); }, onCloseSidebar: this.actions.onCloseSidebar })
    }.width('100%').height('100%')
  }

  @Builder NavigationContent() {
    Navigation(this.navigationStack) { this.RouteContent(AppRoute.ChatHome) }
      .width('100%').height('100%').hideTitleBar(true).mode(NavigationMode.Stack)
      .navDestination(this.NavigationDestination)
  }

  @Builder NavigationDestination(name: string) {
    NavDestination() { this.RouteContent(name as AppRoute) }.hideTitleBar(true).backgroundColor(PAGE_BG)
      .onBackPressed(() => this.actions.onNavigationBack(name as AppRoute))
  }

  @Builder RouteContent(route: AppRoute) {
    Column() {
      if (route === AppRoute.RemoteHome) {
        RemoteHomeView({ pageState: this.remotePageState, isBusy: this.remotePageState.isBusy,
          onOpenSidebar: this.actions.onRemoteHome.openSidebar,
          onConnectWorkspace: this.actions.onRemoteHome.connectWorkspace,
          onAddConnection: this.actions.onRemoteHome.addConnection,
          onOpenRemoteSettings: this.actions.onRemoteHome.openSettings,
          onRefresh: this.actions.onRemoteHome.refresh,
          onShowWorkspaces: this.actions.onRemoteHome.showWorkspaces,
          onShowAssistants: this.actions.onRemoteHome.showAssistants,
          onSelectWorkspace: this.actions.onRemoteHome.selectWorkspace,
          onSelectAssistant: this.actions.onRemoteHome.selectAssistant,
          onCancelWorkspacePicker: this.actions.onRemoteHome.cancelWorkspace,
          onCancelAssistantPicker: this.actions.onRemoteHome.cancelAssistant,
          onSessionQueryChange: this.actions.onRemoteHome.queryChanged,
          onSearchSessions: this.actions.onRemoteHome.search,
          onLoadMoreSessions: this.actions.onRemoteHome.loadMore,
          onReconnect: this.actions.onRemoteHome.reconnect,
          onDisconnect: this.actions.onRemoteHome.disconnect,
          onClearPairing: this.actions.onRemoteHome.clearPairing,
          onCreate: this.actions.onRemoteHome.create,
          onCreateAssistantSession: this.actions.onRemoteHome.createAssistant,
          onCreateInWorkspace: this.actions.onRemoteHome.createInWorkspace,
          onOpenSession: this.actions.onRemoteHome.openSession,
          onDeleteSession: this.actions.onRemoteHome.deleteSession })
      } else if (route === AppRoute.RemoteCreate) {
        RemoteCreateSessionView({
          state: this.remoteCreateState,
          onBack: this.actions.onRemoteCreate.back,
          onToggleDeviceMenu: this.actions.onRemoteCreate.toggleDevices,
          onToggleWorkspaceMenu: this.actions.onRemoteCreate.toggleWorkspaces,
          onSelectDevice: this.actions.onRemoteCreate.selectDevice,
          onSelectWorkspace: (workspace) => this.actions.onRemoteCreate.selectWorkspace(workspace?.path || ''),
          onDraftChange: this.actions.onRemoteCreate.draftChanged,
          onSend: this.actions.onRemoteCreate.send
        })
      } else {
        ConversationViewHost({
          viewState: ConversationViewState.project(route, this.remotePageState, this.generalPageState,
            this.actions.generalStatus()),
          onIntent: (intent: ConversationIntent) => this.actions.onConversationIntent(route, intent)
        })
      }
    }.width('100%').height('100%').backgroundColor(PAGE_BG)
  }

  @Builder SidebarContent() {
    AppSidebar({ sessions: this.generalPageState.recentSessions(), pinnedSessionId: this.generalPageState.pinnedSessionId(),
      selectedSessionId: this.currentRoute === AppRoute.ChatHome || this.currentRoute === AppRoute.GeneralChat ?
        this.generalPageState.activeSession.sessionId : '', connectionState: this.remotePageState.connectionState,
      accountUserId: this.remotePageState.accountUserId,
      activeSection: this.currentRoute === AppRoute.RemoteHome || this.currentRoute === AppRoute.RemoteChat ? 'remote' : 'chat',
      onClose: this.actions.onSidebar.close,
      onNewChat: this.actions.onSidebar.newChat, onEnterCode: this.actions.onSidebar.enterCode,
      onOpenSettings: this.actions.onSidebar.settings, onOpenAccount: this.actions.onSidebar.openAccount,
      onOpenSession: this.actions.onSidebar.openSession,
      onArchiveSession: this.actions.onSidebar.archive, onExportSession: this.actions.onSidebar.exportSession,
      onDeleteSession: this.actions.onSidebar.deleteSession })
  }

  @Builder SettingsContent() {
    if (this.shellState.settingsMode === 'remote' || this.shellState.settingsMode === 'account') {
      RemoteControlSettingsSheet({ desktopName: this.remotePageState.desktopName, desktopId: this.remotePageState.desktopId,
        userId: this.remotePageState.userId, accountUsername: this.remotePageState.accountUsername,
        accountUserId: this.remotePageState.accountUserId, deviceId: this.deviceId,
        controlTargetType: this.remotePageState.controlTargetType,
        controlTargetDeviceId: this.remotePageState.controlTargetDeviceId,
        connectionState: this.remotePageState.connectionState, statusText: this.remotePageState.statusText,
        isBusy: this.remotePageState.isBusy, onClose: this.actions.onSettings.close,
        onAddConnection: this.actions.onSettings.addConnection,
        cloudLogin: this.actions.onSettings.cloudLogin,
        cloudSync: this.actions.onSettings.cloudSync,
        cloudLogout: this.actions.onSettings.cloudLogout,
        cloudListDevices: this.actions.onSettings.cloudListDevices,
        getPermissionMode: this.actions.onSettings.getPermissionMode,
        setPermissionMode: this.actions.onSettings.setPermissionMode,
        openProfileOnAppear: this.shellState.settingsMode === 'account',
        onDisconnect: this.actions.onSettings.disconnect, onReconnect: this.actions.onSettings.reconnect })
    } else {
      SettingsSheet({ generalChatApiUrl: this.generalPageState.apiUrl, generalChatModelName: this.generalPageState.modelName,
        hasGeneralChatApiKey: this.generalPageState.hasApiKey,
        accountUsername: this.remotePageState.accountUsername,
        authenticatedUserId: this.remotePageState.accountUserId,
        deviceId: this.deviceId,
        onOpenAccount: this.actions.onSettings.openAccount,
        onSaveGeneralChatConfig: this.actions.onSettings.saveGeneral,
        onClose: this.actions.onSettings.close })
    }
  }

  @Builder ConnectContent() {
    ConnectView({ remoteUrl: this.remotePageState.remoteUrl, userId: this.remotePageState.userId,
      showRemoteUrlInput: this.remotePageState.showRemoteUrlInput, statusText: this.remotePageState.statusText,
      connectionState: this.remotePageState.connectionState, connectionFailureKind: this.remotePageState.connectionFailureKind,
      isBusy: this.remotePageState.isBusy, isConnected: this.remotePageState.connectionState === 'connected',
      desktopName: this.remotePageState.desktopName, desktopId: this.remotePageState.desktopId, deviceId: this.deviceId,
      accountUserId: this.remotePageState.accountUserId,
      controlTargetDeviceId: this.remotePageState.controlTargetDeviceId,
      requiresAccountAuth: this.remotePageState.requiresAccountAuth, accountUsername: this.remotePageState.accountUsername,
      startWithScanner: true,
      onBack: this.actions.onConnect.back, onConnect: this.actions.onConnect.connect,
      onClearPairing: this.actions.onConnect.clearPairing, onRemoteUrlChange: this.actions.onConnect.urlChanged,
      onUserIdChange: this.actions.onConnect.userChanged, onRemoteUrlDetected: this.actions.onConnect.detected,
      onRemoteUrlInputVisibleChange: this.actions.onConnect.inputVisible,
      onPasteRemoteUrl: this.actions.onConnect.paste, onScanRemoteUrl: this.actions.onConnect.scan,
      cloudListDevices: this.actions.onConnect.cloudListDevices,
      cloudSelectDevice: this.actions.onConnect.cloudSelectDevice })
      .width('100%').height('100%')
  }
}

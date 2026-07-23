import { RemoteSession } from '../../model/RemoteModels';
import { AppShell } from './AppShell';
import { AppSidebar } from './AppSidebar';
import { ConnectView } from './ConnectView';
import { ConversationIntent } from './ConversationIntent';
import { ConversationViewHost } from './ConversationViewHost';
import { RemoteControlSettingsSheet } from './RemoteControlSettingsSheet';
import { RemoteHomeView } from './RemoteHomeView';
import { SettingsSheet } from './SettingsSheet';
import { PAGE_BG } from './Theme';
import { AppNavigationBackAction, AppRoute } from '../navigation/AppRouteContract';
import { AppShellState } from '../state/AppShellState';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemotePageState } from '../state/RemotePageState';
import { ConversationViewState } from '../state/ConversationViewState';

export class AppRootPresentationActions {
  readonly onNavigationBack: (route: AppRoute) => boolean;
  readonly onConversationIntent: (route: AppRoute, intent: ConversationIntent) => void;
  readonly onCloseSidebar: () => void;
  readonly onRemoteHome: RemoteHomePresentationActions;
  readonly onSidebar: SidebarPresentationActions;
  readonly onSettings: SettingsPresentationActions;
  readonly onConnect: ConnectPresentationActions;
  readonly generalStatus: () => string;

  constructor(
    onNavigationBack: (route: AppRoute) => boolean,
    onConversationIntent: (route: AppRoute, intent: ConversationIntent) => void,
    onCloseSidebar: () => void,
    onRemoteHome: RemoteHomePresentationActions,
    onSidebar: SidebarPresentationActions,
    onSettings: SettingsPresentationActions,
    onConnect: ConnectPresentationActions,
    generalStatus: () => string
  ) {
    this.onNavigationBack = onNavigationBack;
    this.onConversationIntent = onConversationIntent;
    this.onCloseSidebar = onCloseSidebar;
    this.onRemoteHome = onRemoteHome;
    this.onSidebar = onSidebar;
    this.onSettings = onSettings;
    this.onConnect = onConnect;
    this.generalStatus = generalStatus;
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
  readonly createInWorkspace: (path: string) => void; readonly openSession: (session: RemoteSession) => void;
  readonly deleteSession: (session: RemoteSession) => void;

  constructor(
    openSidebar: () => void, connectWorkspace: () => void, addConnection: () => void, openSettings: () => void,
    refresh: () => void, showWorkspaces: () => void, showAssistants: () => void,
    selectWorkspace: (path: string) => void, selectAssistant: (path: string) => void,
    cancelWorkspace: () => void, cancelAssistant: () => void, queryChanged: (query: string) => void,
    search: () => void, loadMore: () => void, reconnect: () => void, disconnect: () => void,
    clearPairing: () => void, create: (agentType: string) => void, createAssistant: () => void,
    createInWorkspace: (path: string) => void, openSession: (session: RemoteSession) => void,
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
  readonly settings: () => void; readonly openSession: (session: RemoteSession) => void;
  readonly archive: (session: RemoteSession, archived: boolean) => void;
  readonly exportSession: (session: RemoteSession) => void; readonly deleteSession: (session: RemoteSession) => void;
  constructor(
    close: () => void, newChat: () => void, enterCode: () => void, settings: () => void,
    openSession: (session: RemoteSession) => void, archive: (session: RemoteSession, archived: boolean) => void,
    exportSession: (session: RemoteSession) => void, deleteSession: (session: RemoteSession) => void
  ) {
    this.close = close; this.newChat = newChat; this.enterCode = enterCode; this.settings = settings;
    this.openSession = openSession; this.archive = archive; this.exportSession = exportSession; this.deleteSession = deleteSession;
  }
}

export class SettingsPresentationActions {
  readonly close: () => void; readonly addConnection: () => void; readonly disconnect: () => void;
  readonly reconnect: () => void;
  readonly saveGeneral: (url: string, key: string, model: string, clear: boolean) => Promise<string>;
  constructor(
    close: () => void, addConnection: () => void, disconnect: () => void, reconnect: () => void,
    saveGeneral: (url: string, key: string, model: string, clear: boolean) => Promise<string>
  ) {
    this.close = close; this.addConnection = addConnection; this.disconnect = disconnect;
    this.reconnect = reconnect; this.saveGeneral = saveGeneral;
  }
}

export class ConnectPresentationActions {
  readonly back: () => void; readonly connect: (password?: string) => void; readonly clearPairing: () => void;
  readonly urlChanged: (url: string) => void; readonly userChanged: (user: string) => void;
  readonly detected: (url: string) => boolean; readonly inputVisible: (visible: boolean) => void;
  readonly paste: () => void; readonly scan: () => void;
  constructor(
    back: () => void, connect: (password?: string) => void, clearPairing: () => void,
    urlChanged: (url: string) => void, userChanged: (user: string) => void,
    detected: (url: string) => boolean, inputVisible: (visible: boolean) => void,
    paste: () => void, scan: () => void
  ) {
    this.back = back; this.connect = connect; this.clearPairing = clearPairing;
    this.urlChanged = urlChanged; this.userChanged = userChanged; this.detected = detected;
    this.inputVisible = inputVisible; this.paste = paste; this.scan = scan;
  }
}

@ComponentV2
export struct AppRootPresentation {
  @Param shellState: AppShellState = new AppShellState();
  @Param navigationStack: NavPathStack = new NavPathStack();
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param generalPageState: GeneralChatPageState = new GeneralChatPageState();
  @Param deviceId: string = '';
  @Param currentRoute: AppRoute = AppRoute.ChatHome;
  @Param actions: AppRootPresentationActions = new AppRootPresentationActions(
    () => false, () => {}, () => {},
    new RemoteHomePresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {},
      () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {},
      () => {}, () => {}, () => {}, () => {}, () => {}),
    new SidebarPresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}, () => {}),
    new SettingsPresentationActions(() => {}, () => {}, () => {}, () => {}, async () => ''),
    new ConnectPresentationActions(() => {}, () => {}, () => {}, () => {}, () => {}, () => false, () => {}, () => {}, () => {}),
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
      activeSection: this.currentRoute === AppRoute.RemoteHome || this.currentRoute === AppRoute.RemoteChat ? 'remote' : 'chat',
      onClose: this.actions.onSidebar.close,
      onNewChat: this.actions.onSidebar.newChat, onEnterCode: this.actions.onSidebar.enterCode,
      onOpenSettings: this.actions.onSidebar.settings, onOpenSession: this.actions.onSidebar.openSession,
      onArchiveSession: this.actions.onSidebar.archive, onExportSession: this.actions.onSidebar.exportSession,
      onDeleteSession: this.actions.onSidebar.deleteSession })
  }

  @Builder SettingsContent() {
    if (this.shellState.settingsMode === 'remote') {
      RemoteControlSettingsSheet({ desktopName: this.remotePageState.desktopName, desktopId: this.remotePageState.desktopId,
        userId: this.remotePageState.userId, accountUsername: this.remotePageState.accountUsername,
        authenticatedUserId: this.remotePageState.authenticatedUserId, deviceId: this.deviceId,
        connectionState: this.remotePageState.connectionState, statusText: this.remotePageState.statusText,
        isBusy: this.remotePageState.isBusy, onClose: this.actions.onSettings.close,
        onAddConnection: this.actions.onSettings.addConnection,
        onDisconnect: this.actions.onSettings.disconnect, onReconnect: this.actions.onSettings.reconnect })
    } else {
      SettingsSheet({ generalChatApiUrl: this.generalPageState.apiUrl, generalChatModelName: this.generalPageState.modelName,
        hasGeneralChatApiKey: this.generalPageState.hasApiKey,
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
      requiresAccountAuth: this.remotePageState.requiresAccountAuth, accountUsername: this.remotePageState.accountUsername,
      startWithScanner: true,
      onBack: this.actions.onConnect.back, onConnect: this.actions.onConnect.connect,
      onClearPairing: this.actions.onConnect.clearPairing, onRemoteUrlChange: this.actions.onConnect.urlChanged,
      onUserIdChange: this.actions.onConnect.userChanged, onRemoteUrlDetected: this.actions.onConnect.detected,
      onRemoteUrlInputVisibleChange: this.actions.onConnect.inputVisible,
      onPasteRemoteUrl: this.actions.onConnect.paste, onScanRemoteUrl: this.actions.onConnect.scan })
      .width('100%').height('100%')
  }
}

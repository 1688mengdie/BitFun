@ObservedV2
export class AppShellState {
  @Trace showSidebar: boolean = false;
  @Trace showSettings: boolean = false;
  @Trace settingsMode: string = 'general';
  @Trace showConnectSheet: boolean = false;
  private accountReturnMode: string = '';

  setSidebarVisible(visible: boolean): void {
    this.showSidebar = visible;
  }

  setSettingsVisible(visible: boolean): void {
    this.showSettings = visible;
    if (!visible) {
      this.accountReturnMode = '';
    }
  }

  openSettings(mode: string): void {
    if (mode === 'account') {
      this.accountReturnMode = this.showSettings ? this.settingsMode : '';
    } else {
      this.accountReturnMode = '';
    }
    this.settingsMode = mode;
    this.showSettings = true;
  }

  leaveSettings(): void {
    if (this.settingsMode === 'account' && this.accountReturnMode.length > 0) {
      this.settingsMode = this.accountReturnMode;
      this.accountReturnMode = '';
      this.showSettings = true;
      return;
    }
    this.setSettingsVisible(false);
  }

  setConnectSheetVisible(visible: boolean): void {
    this.showConnectSheet = visible;
  }

  closeGlobalSurfaces(): void {
    this.showSidebar = false;
    this.showSettings = false;
    this.accountReturnMode = '';
    this.showConnectSheet = false;
  }
}

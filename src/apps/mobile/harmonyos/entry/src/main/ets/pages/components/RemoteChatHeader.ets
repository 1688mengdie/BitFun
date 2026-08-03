import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ConversationUiSession } from './ConversationUiModels';
import { ACCENT, CARD, INK, LINE, MUTED, PAGE_BG, PRIMARY_ACTION_TEXT, SOFT } from './Theme';
import { SidebarToggleButton } from './SidebarToggleButton';

@ComponentV2
export struct RemoteChatHeader {
  @Param activeSession: ConversationUiSession = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  @Param workspaceBranch: string = '';
  @Param desktopName: string = '';
  @Param showBackButton: boolean = true;
  @Param showSidebarRestoreButton: boolean = false;
  @Event onBack: () => void = () => {};
  @Event onRestoreSidebar: () => void = () => {};
  @Event onOpenActions: () => void = () => {};
  @Event onRenameSession: (title: string) => void = (_title: string) => {};
  @Local showTitleEditor: boolean = false;
  @Local renameTitle: string = '';

  build() {
    Column() {
      this.HeaderRow()
      if (this.showTitleEditor) {
        this.TitleEditor()
      }
    }
    .width('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  HeaderRow() {
    Row({ space: 8 }) {
      this.LeadingControl()
      Column({ space: 3 }) {
        Text(this.activeSession.title || RemoteI18n.t('chat.remoteSession'))
          .fontSize(18)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .onClick(() => {
            this.renameTitle = this.activeSession.title || '';
            this.showTitleEditor = true;
          })
        Row({ space: 6 }) {
          Text(this.headerContextTitle())
            .fontSize(14)
            .fontColor(MUTED)
            .textAlign(TextAlign.Center)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .width('100%')
        .justifyContent(FlexAlign.Center)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Center)
      Stack({ alignContent: Alignment.Center }) {
        Text('•••')
          .fontSize(13)
          .fontColor(INK)
      }
      .width(48)
      .height(48)
      .backgroundColor(CARD)
      .borderRadius(24)
      .border({ width: 1, color: LINE })
      .accessibilityText(RemoteI18n.t('sidebar.more'))
      .onClick(() => {
        this.showTitleEditor = false;
        this.onOpenActions();
      })
    }
    .width('100%')
    .alignItems(VerticalAlign.Center)
    .padding({ left: 18, right: 18, top: 14, bottom: 16 })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  private LeadingControl() {
    if (this.showSidebarRestoreButton) {
      SidebarToggleButton({
        restore: true,
        controlSize: 48,
        onToggle: this.onRestoreSidebar
      })
    } else if (this.showBackButton) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.chevron_left'))
          .fontSize(22)
          .fontColor([INK])
      }
      .width(48)
      .height(48)
      .backgroundColor(CARD)
      .border({ width: 1, color: LINE })
      .borderRadius(24)
      .accessibilityText(RemoteI18n.t('common.back'))
      .onClick(() => {
        this.onBack();
      })
    } else {
      Blank().width(48).height(48)
    }
  }

  @Builder
  TitleEditor() {
    Row({ space: 8 }) {
      TextInput({ placeholder: RemoteI18n.t('chat.sessionTitle'), text: this.renameTitle })
        .layoutWeight(1)
        .height(42)
        .fontSize(14)
        .backgroundColor(CARD)
        .borderRadius(14)
        .padding({ left: 12, right: 12 })
        .border({ width: 1, color: LINE })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.renameTitle = value;
        })
      Text(RemoteI18n.t('common.save'))
        .width(52)
        .height(42)
        .fontSize(13)
        .fontColor(PRIMARY_ACTION_TEXT)
        .textAlign(TextAlign.Center)
        .backgroundColor(this.renameTitle.trim().length > 0 ? ACCENT : SOFT)
        .borderRadius(14)
        .onClick(() => {
          if (this.renameTitle.trim().length > 0) {
            this.onRenameSession(this.renameTitle.trim());
            this.showTitleEditor = false;
          }
        })
      Text(RemoteI18n.t('common.cancel'))
        .width(52)
        .height(42)
        .fontSize(13)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .backgroundColor(SOFT)
        .borderRadius(14)
        .onClick(() => {
          this.showTitleEditor = false;
        })
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 10, bottom: 8 })
    .backgroundColor(PAGE_BG)
  }

  private headerContextTitle(): string {
    if (this.desktopName.length > 0) {
      return this.desktopName;
    }
    const brand = 'BitFun';
    return this.workspaceBranch.length > 0 ? `${brand} · ${this.workspaceBranch}` : brand;
  }

}

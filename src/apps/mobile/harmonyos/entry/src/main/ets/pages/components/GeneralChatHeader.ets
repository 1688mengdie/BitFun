import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CARD, INK, LINE, PAGE_BG } from './Theme';
import { SidebarToggleButton } from './SidebarToggleButton';

@ComponentV2
export struct GeneralChatHeader {
  @Param title: string = '';
  @Param showActions: boolean = false;
  @Param showSidebarButton: boolean = true;
  @Param showBackButton: boolean = false;
  @Param showSidebarRestoreButton: boolean = false;
  @Event onOpenSidebar: () => void = () => {};
  @Event onRestoreSidebar: () => void = () => {};
  @Event onBack: () => void = () => {};
  @Event onOpenActions: () => void = () => {};

  build() {
    Row({ space: 8 }) {
      this.LeadingControl()

      Text(this.title || 'BitFun')
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)

      this.TrailingControl()
    }
    .width('100%')
    .height(64)
    .alignItems(VerticalAlign.Center)
    .padding({ left: 16, right: 16, top: 8, bottom: 8 })
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
    } else if (this.showSidebarButton) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.line_3_horizontal'))
          .fontSize(22)
          .fontColor([INK])
      }
      .width(48)
      .height(48)
      .backgroundColor(CARD)
      .border({ width: 1, color: LINE })
      .borderRadius(24)
      .accessibilityText(RemoteI18n.t('sidebar.more'))
      .onClick(() => {
        this.onOpenSidebar();
      })
    } else {
      Blank().width(48).height(48)
    }
  }

  @Builder
  private TrailingControl() {
    if (this.showActions) {
      Stack({ alignContent: Alignment.Center }) {
        Text('•••')
          .fontSize(13)
          .fontColor(INK)
      }
      .width(48)
      .height(48)
      .backgroundColor(CARD)
      .border({ width: 1, color: LINE })
      .borderRadius(24)
      .accessibilityText(RemoteI18n.t('sidebar.more'))
      .onClick(() => {
        this.onOpenActions();
      })
    } else {
      Blank().width(48).height(48)
    }
  }
}

import { ConnectionStatusPresenter } from '../../services/ConnectionStatusPresenter';
import { CARD, GREEN, INK, MUTED, RED } from './Theme';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CompactMenuButton } from './CompactMenuButton';

@ComponentV2
export struct RemoteHeader {
  @Param desktopName: string = '';
  @Param connectionState: string = 'idle';
  @Param isLoading: boolean = false;
  @Event onOpenSidebar: () => void = () => {};
  @Event onOpenActions: () => void = () => {};
  @Event onOpenDevices: () => void = () => {};

  build() {
    Row() {
      this.MenuButton()
      Column({ space: 2 }) {
        Text(RemoteI18n.t('remote.title'))
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .maxLines(1)
        Row({ space: 6 }) {
          if (this.isLoading || this.isConnecting()) {
            LoadingProgress()
              .width(14)
              .height(14)
              .color(MUTED)
          } else {
            Text('')
              .width(7)
              .height(7)
              .backgroundColor(this.connectionColor())
              .borderRadius(4)
          }
          SymbolGlyph($r('sys.symbol.desktop'))
            .fontSize(16)
            .fontColor([MUTED])
            .width(19)
            .height(18)
          Text(this.desktopName || RemoteI18n.t('remote.settings.noDesktop'))
            .fontSize(13)
            .fontColor(MUTED)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
          SymbolGlyph($r('sys.symbol.chevron_down'))
            .fontSize(11)
            .fontColor([MUTED])
            .width(13)
            .height(13)
            .opacity(0.58)
        }
        .height(28)
        .padding({ left: 8, right: 8 })
        .borderRadius(10)
        .accessibilityText(RemoteI18n.t('connect.accountDevicesTitle'))
        .onClick(() => {
          this.onOpenDevices();
        })
      }
      .layoutWeight(1)
      .height(52)
      .alignItems(HorizontalAlign.Center)
      .justifyContent(FlexAlign.Center)
      this.MoreButton()
    }
    .width('100%')
    .height(68)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private MenuButton() {
    CompactMenuButton({
      onOpen: () => {
        this.onOpenSidebar();
      }
    })
  }

  @Builder
  private MoreButton() {
    Stack({ alignContent: Alignment.Center }) {
      Row({ space: 4 }) {
        Text('').width(4).height(4).backgroundColor(MUTED).borderRadius(2)
        Text('').width(4).height(4).backgroundColor(MUTED).borderRadius(2)
        Text('').width(4).height(4).backgroundColor(MUTED).borderRadius(2)
      }
      .height(10)
      .alignItems(VerticalAlign.Center)
    }
    .width(48)
    .height(48)
    .backgroundColor(CARD)
    .borderRadius(24)
    .shadow({ radius: 18, color: '#10000000', offsetY: 8 })
    .onClick(() => {
      this.onOpenActions();
    })
  }

  private connectionColor(): ResourceColor {
    const tone = ConnectionStatusPresenter.tone(this.connectionState);
    if (tone === 'ok') {
      return GREEN;
    }
    if (tone === 'error') {
      return RED;
    }
    return MUTED;
  }

  private isConnecting(): boolean {
    return this.connectionState === 'parsing' ||
      this.connectionState === 'pairing' ||
      this.connectionState === 'reconnecting';
  }
}

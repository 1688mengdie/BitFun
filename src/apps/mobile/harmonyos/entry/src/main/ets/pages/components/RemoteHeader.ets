import { ConnectionStatusPresenter } from '../../services/ConnectionStatusPresenter';
import { CARD, GREEN, INK, MUTED, RED } from './Theme';
import { RemoteI18n } from '../../i18n/RemoteI18n';

@ComponentV2
export struct RemoteHeader {
  @Param desktopName: string = '';
  @Param connectionState: string = 'idle';
  @Param isLoading: boolean = false;
  @Event onOpenSidebar: () => void = () => {};
  @Event onOpenActions: () => void = () => {};

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
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Center)
      this.MoreButton()
    }
    .width('100%')
    .height(68)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  private MenuButton() {
    Stack({ alignContent: Alignment.Center }) {
      SymbolGlyph($r('sys.symbol.line_3_horizontal'))
        .fontSize(22)
        .fontColor([INK])
        .width(24)
        .height(24)
    }
    .width(48)
    .height(48)
    .backgroundColor(CARD)
    .borderRadius(24)
    .shadow({ radius: 18, color: '#10000000', offsetY: 8 })
    .onClick(() => {
      this.onOpenSidebar();
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

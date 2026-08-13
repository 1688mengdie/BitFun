import { describe, it, expect } from '@ohos/hypium';
import { RemoteI18n } from '../main/ets/i18n/RemoteI18n';
import { ChatComposerPolicy, ComposerPrimaryAction } from '../main/ets/services/ChatComposerPolicy';
import { ChatSurface } from '../main/ets/pages/components/ChatSurface';
import {
  GENERAL_CHAT_COMPOSER_CAPABILITIES,
  REMOTE_CHAT_COMPOSER_CAPABILITIES
} from '../main/ets/pages/components/ChatComposerCapabilities';
import { ConversationViewState } from '../main/ets/pages/state/ConversationViewState';
import { GeneralChatPageState } from '../main/ets/pages/state/GeneralChatPageState';
import { RemotePageState } from '../main/ets/pages/state/RemotePageState';
import { AppRoute } from '../main/ets/pages/navigation/AppRouteContract';
import { ThinkingPresentationPolicy } from '../main/ets/pages/policy/ThinkingPresentationPolicy';
import {
  ConnectSheetLandingPolicy,
  ConnectSheetStatusPolicy,
  ConnectSheetStep
} from '../main/ets/pages/policy/ConnectSheetLandingPolicy';
import {
  RemoteConnectEntryKind,
  RemoteSurfaceEntryPolicy
} from '../main/ets/pages/policy/RemoteSurfaceEntryPolicy';
import { CONNECT_INTENT_AUTO, CONNECT_INTENT_SCAN } from '../main/ets/pages/state/AppShellState';
import {
  ConnectScanDecisionPolicy,
  DetectedUrlAction
} from '../main/ets/services/ConnectScanDecisionPolicy';

export default function conversationPresentationUnitTest() {
  describe('Local conversation empty home', () => {
    it('keeps local chat on a blank timeline instead of suggestion chips', 0, () => {
      const projection = ConversationViewState.project(
        AppRoute.ChatHome,
        new RemotePageState(),
        new GeneralChatPageState(),
        ''
      );

      expect(projection.surface).assertEqual(ChatSurface.General);
      expect(projection.showSuggestionsWhenEmpty).assertFalse();
      expect(projection.timelineItems.length).assertEqual(0);
    });
  });

  describe('ThinkingPresentationPolicy', () => {
    it('treats active and running as in-progress thinking', 0, () => {
      expect(ThinkingPresentationPolicy.isRunning('active')).assertTrue();
      expect(ThinkingPresentationPolicy.isRunning('running')).assertTrue();
      expect(ThinkingPresentationPolicy.isRunning('ACTIVE')).assertTrue();
      expect(ThinkingPresentationPolicy.isRunning('complete')).assertFalse();
      expect(ThinkingPresentationPolicy.isRunning('')).assertFalse();
    });

    it('hides the block when thinking is done and the body is empty', 0, () => {
      expect(ThinkingPresentationPolicy.shouldRender('complete', '')).assertFalse();
      expect(ThinkingPresentationPolicy.shouldRender('complete', '   ')).assertFalse();
    });

    it('keeps the block visible while running or when a body remains', 0, () => {
      expect(ThinkingPresentationPolicy.shouldRender('running', '')).assertTrue();
      expect(ThinkingPresentationPolicy.shouldRender('complete', '先看现有会话结构')).assertTrue();
    });

    it('expands by default only while running unless keepExpandedWhenDone is set', 0, () => {
      expect(ThinkingPresentationPolicy.defaultExpanded(true, false)).assertTrue();
      expect(ThinkingPresentationPolicy.defaultExpanded(false, false)).assertFalse();
      expect(ThinkingPresentationPolicy.defaultExpanded(false, true)).assertTrue();
    });
  });

  describe('ChatComposerPolicy add button', () => {
    it('hides the add control unless attachments are supported', 0, () => {
      expect(ChatComposerPolicy.shouldShowAddButton(true, false)).assertFalse();
      expect(ChatComposerPolicy.shouldShowAddButton(false, true)).assertFalse();
      expect(ChatComposerPolicy.shouldShowAddButton(true, true)).assertTrue();
    });

    it('lets local chat pick images because the composer can honour attachments', 0, () => {
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.surface).assertEqual(ChatSurface.General);
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.supportsAttachments).assertTrue();
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.showAddButton).assertTrue();
      expect(ChatComposerPolicy.shouldShowAddButton(
        GENERAL_CHAT_COMPOSER_CAPABILITIES.showAddButton,
        GENERAL_CHAT_COMPOSER_CAPABILITIES.supportsAttachments
      )).assertTrue();
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.showAddButton).assertTrue();
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.supportsAttachments).assertTrue();
    });

    it('names the primary composer action for assistive labels', 0, () => {
      expect(ChatComposerPolicy.primaryActionAccessibilityKey(ComposerPrimaryAction.Send, false))
        .assertEqual('chat.send');
      expect(ChatComposerPolicy.primaryActionAccessibilityKey(ComposerPrimaryAction.Stop, false))
        .assertEqual('chat.stop');
      expect(ChatComposerPolicy.primaryActionAccessibilityKey(ComposerPrimaryAction.Stop, true))
        .assertEqual('chat.stopListening');
      expect(ChatComposerPolicy.primaryActionAccessibilityKey(ComposerPrimaryAction.Voice, false))
        .assertEqual('chat.voiceInput');
    });
  });

  describe('Conversation source naming', () => {
    it('uses the same local/remote pair across the switcher and remote chrome', 0, () => {
      expect(RemoteI18n.t('sidebar.local')).assertEqual('本地');
      expect(RemoteI18n.t('sidebar.code')).assertEqual('远程');
      expect(RemoteI18n.t('sidebar.code')).assertEqual(RemoteI18n.t('remote.title'));
      expect(RemoteI18n.t('connect.accountDevicesSubtitle')).assertEqual('远程');
      expect(RemoteI18n.t('chat.remoteSession')).assertEqual('远程会话');
    });

    it('does not keep leftover pending-copy keys that claim general chat is unavailable', 0, () => {
      expect(RemoteI18n.t('chatHome.generalPending')).assertEqual('chatHome.generalPending');
      expect(RemoteI18n.t('chatHome.generalAdvicePending')).assertEqual('chatHome.generalAdvicePending');
    });
  });

  describe('ConnectSheetLandingPolicy', () => {
    it('opens unsigned-in automatic entry on the explainer, not the camera', 0, () => {
      expect(ConnectSheetLandingPolicy.initialStep(CONNECT_INTENT_AUTO, false))
        .assertEqual(ConnectSheetStep.Intro);
      expect(ConnectSheetLandingPolicy.initialStep(CONNECT_INTENT_AUTO, true))
        .assertEqual(ConnectSheetStep.Account);
      expect(ConnectSheetLandingPolicy.initialStep(CONNECT_INTENT_SCAN, false))
        .assertEqual(ConnectSheetStep.Scan);
      expect(ConnectSheetLandingPolicy.initialStep(CONNECT_INTENT_SCAN, true))
        .assertEqual(ConnectSheetStep.Scan);
    });

    it('keeps the current pairing step while a URL is present or pairing is busy', 0, () => {
      expect(ConnectSheetLandingPolicy.visibleStep(ConnectSheetStep.Intro, false))
        .assertEqual(ConnectSheetStep.Intro);
      expect(ConnectSheetLandingPolicy.visibleStep(ConnectSheetStep.Scan, false))
        .assertEqual(ConnectSheetStep.Scan);
      expect(ConnectSheetLandingPolicy.visibleStep(ConnectSheetStep.Account, true))
        .assertEqual(ConnectSheetStep.Account);
      expect(ConnectSheetLandingPolicy.visibleStep(ConnectSheetStep.Account, false))
        .assertEqual(ConnectSheetStep.Intro);
    });

    it('maps a failed pair to the matching reconnect hint', 0, () => {
      expect(ConnectSheetStatusPolicy.failureHintKey('expired_room'))
        .assertEqual('connect.hintExpiredRoom');
      expect(ConnectSheetStatusPolicy.failureHintKey('network'))
        .assertEqual('connect.hintNetwork');
      expect(ConnectSheetStatusPolicy.isConnectError('failed', false, false, '', '失败', '', ''))
        .assertTrue();
      expect(ConnectSheetStatusPolicy.isConnectError('pairing', true, false, '', '失败', '', ''))
        .assertFalse();
    });
  });

  describe('RemoteSurfaceEntryPolicy', () => {
    it('treats switching to remote as navigation and keeps connect as an explicit action', 0, () => {
      expect(RemoteSurfaceEntryPolicy.shouldOpenConnectSheet(RemoteConnectEntryKind.EnterSurface))
        .assertFalse();
      expect(RemoteSurfaceEntryPolicy.shouldOpenConnectSheet(RemoteConnectEntryKind.OpenConnect))
        .assertTrue();
      expect(RemoteSurfaceEntryPolicy.shouldOpenConnectSheet(RemoteConnectEntryKind.OpenScan))
        .assertTrue();
    });
  });

  describe('ConnectScanDecisionPolicy', () => {
    it('pairs an ordinary QR immediately without asking for an account password', 0, () => {
      expect(ConnectScanDecisionPolicy.decide(false, false, '', '', ''))
        .assertEqual(DetectedUrlAction.PAIR_NOW);
      expect(ConnectScanDecisionPolicy.decide(false, true, 'alice', '', 'desktop-1'))
        .assertEqual(DetectedUrlAction.PAIR_NOW);
    });

    it('asks for a password only when the QR needs account auth and this phone is signed out', 0, () => {
      expect(ConnectScanDecisionPolicy.decide(true, false, '', 'alice', 'desktop-1'))
        .assertEqual(DetectedUrlAction.PROMPT_ACCOUNT_PASSWORD);
    });

    it('reuses the signed-in cloud session instead of asking for the password again', 0, () => {
      expect(ConnectScanDecisionPolicy.decide(true, true, 'alice', 'alice', 'desktop-1'))
        .assertEqual(DetectedUrlAction.USE_CLOUD_DEVICE);
      expect(ConnectScanDecisionPolicy.decide(true, true, 'alice', 'alice', ''))
        .assertEqual(DetectedUrlAction.SHOW_CLOUD_DEVICES);
      expect(ConnectScanDecisionPolicy.decide(true, true, 'alice', '', 'desktop-1'))
        .assertEqual(DetectedUrlAction.USE_CLOUD_DEVICE);
    });

    it('still asks for a password when the QR belongs to a different account', 0, () => {
      expect(ConnectScanDecisionPolicy.decide(true, true, 'alice', 'bob', 'desktop-1'))
        .assertEqual(DetectedUrlAction.PROMPT_ACCOUNT_PASSWORD);
    });
  });
}

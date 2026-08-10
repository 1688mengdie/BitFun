// Each offered action has a live and a dimmed form, and both are worth keeping
// apart from Idle. SendBlocked means the user has written something the link
// cannot carry yet — drawing that as idle would tell them their draft does not
// exist. VoiceBlocked keeps the microphone on screen through a busy moment, so
// the control does not swap glyphs mid-turn and then swap back. Idle is the one
// state with nothing to offer: no dictation on this surface, and no draft.
export enum ComposerPrimaryAction {
  Stop = 'stop',
  Send = 'send',
  SendBlocked = 'send_blocked',
  Voice = 'voice',
  VoiceBlocked = 'voice_blocked',
  Idle = 'idle'
}

export class ChatComposerPolicy {
  static canSend(
    text: string,
    attachmentCount: number,
    isBusy: boolean,
    requiresRemoteConnection: boolean,
    connectionState: string
  ): boolean {
    const hasContent = text.trim().length > 0 || attachmentCount > 0;
    const remoteAvailable = !requiresRemoteConnection ||
      connectionState === 'connected' || connectionState === 'reconnecting';
    return hasContent && !isBusy && remoteAvailable;
  }

  static canUseVoice(text: string, attachmentCount: number, isBusy: boolean): boolean {
    return text.trim().length === 0 && attachmentCount === 0 && !isBusy;
  }

  // Which of the things the one round button on the right is offering. The
  // composer has a single primary slot rather than a row of buttons, so "which
  // action" is a decision, not a layout detail — and it is the same decision on
  // both clients. `isStopping` folds in whatever locally outranks the draft: a
  // running turn on both clients, plus dictation on this one.
  static primaryAction(
    text: string,
    attachmentCount: number,
    isBusy: boolean,
    isStopping: boolean,
    requiresRemoteConnection: boolean,
    connectionState: string,
    showVoiceInput: boolean
  ): ComposerPrimaryAction {
    if (isStopping) {
      return ComposerPrimaryAction.Stop;
    }
    if (text.trim().length > 0 || attachmentCount > 0) {
      const sendable = ChatComposerPolicy.canSend(
        text, attachmentCount, isBusy, requiresRemoteConnection, connectionState);
      return sendable ? ComposerPrimaryAction.Send : ComposerPrimaryAction.SendBlocked;
    }
    if (!showVoiceInput) {
      return ComposerPrimaryAction.Idle;
    }
    const usable = ChatComposerPolicy.canUseVoice(text, attachmentCount, isBusy);
    return usable ? ComposerPrimaryAction.Voice : ComposerPrimaryAction.VoiceBlocked;
  }

  // Whether the bar is in its tall form, where the field gets its own row and
  // the side controls move to a second one. A newline counts even without
  // focus: the collapsed field is one line tall, and text the user cannot see
  // is text they cannot check before sending.
  static isExpanded(
    text: string,
    inputFocused: boolean,
    quickActionsOpen: boolean,
    modelSelectorOpen: boolean
  ): boolean {
    return inputFocused || quickActionsOpen || modelSelectorOpen || text.indexOf('\n') >= 0;
  }
}

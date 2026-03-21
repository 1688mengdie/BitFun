/**
 * Copy-dialog event handling for FlowChat.
 */

import { useEffect } from 'react';
import { globalEventBus } from '@/infrastructure/event-bus';
import { notificationService } from '@/shared/notification-system';
import { getElementText, copyTextToClipboard } from '@/shared/utils/textSelection';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useFlowChatCopyDialog');

export function useFlowChatCopyDialog(): void {
  useEffect(() => {
    const unsubscribe = globalEventBus.on('flowchat:copy-dialog', ({ dialogTurn }) => {
      if (!dialogTurn) {
        log.warn('Copy failed: dialog element not provided');
        return;
      }

      const dialogElement = dialogTurn as HTMLElement;
      const fullText = getElementText(dialogElement);

      if (!fullText || fullText.trim().length === 0) {
        notificationService.warning('Dialog is empty, nothing to copy');
        return;
      }

      copyTextToClipboard(fullText).then(success => {
        if (!success) {
          notificationService.error('Copy failed. Please try again.');
        }
      });
    });

    return unsubscribe;
  }, []);
}

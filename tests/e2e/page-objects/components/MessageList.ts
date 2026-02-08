/**
 * Page object for message list (chat messages area).
 */
import { BasePage } from '../BasePage';
import { browser, $$ } from '@wdio/globals';
import { waitForStreamingComplete } from '../../helpers/wait-utils';
import { environmentSettings } from '../../config/capabilities';

export class MessageList extends BasePage {
  private selectors = {
    container: '[data-testid="message-list"]',
    userMessage: '[data-testid^="user-message-"]',
    modelResponse: '[data-testid^="model-response-"]',
    messageItem: '[data-testid^="message-item-"]',
    codeBlock: '[data-testid="code-block"]',
    toolCard: '[data-testid^="tool-card-"]',
    emptyState: '[data-testid="chat-empty-state"]',
    scrollToBottom: '[data-testid="scroll-to-bottom"]',
  };

  async isVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.container);
  }

  async waitForLoad(): Promise<void> {
    await this.waitForElement(this.selectors.container);
  }

  async isEmpty(): Promise<boolean> {
    return this.isElementVisible(this.selectors.emptyState);
  }

  async getUserMessages(): Promise<WebdriverIO.Element[]> {
    return $$(this.selectors.userMessage);
  }

  async getModelResponses(): Promise<WebdriverIO.Element[]> {
    return $$(this.selectors.modelResponse);
  }

  async getUserMessageCount(): Promise<number> {
    const messages = await this.getUserMessages();
    return messages.length;
  }

  async getModelResponseCount(): Promise<number> {
    const responses = await this.getModelResponses();
    return responses.length;
  }

  async getTotalMessageCount(): Promise<number> {
    const messages = await $$(this.selectors.messageItem);
    return messages.length;
  }

  async getLastUserMessageText(): Promise<string> {
    const messages = await this.getUserMessages();
    if (messages.length === 0) {
      return '';
    }
    return messages[messages.length - 1].getText();
  }

  async getLastModelResponseText(): Promise<string> {
    const responses = await this.getModelResponses();
    if (responses.length === 0) {
      return '';
    }
    return responses[responses.length - 1].getText();
  }

  async waitForNewMessage(
    currentCount: number,
    timeout: number = environmentSettings.defaultTimeout
  ): Promise<void> {
    await browser.waitUntil(
      async () => {
        const newCount = await this.getTotalMessageCount();
        return newCount > currentCount;
      },
      {
        timeout,
        timeoutMsg: `No new message appeared within ${timeout}ms`,
      }
    );
  }

  async waitForResponseComplete(
    timeout: number = environmentSettings.streamingResponseTimeout
  ): Promise<void> {
    await waitForStreamingComplete(this.selectors.modelResponse, 2000, timeout);
  }

  async getCodeBlocks(): Promise<WebdriverIO.Element[]> {
    return $$(this.selectors.codeBlock);
  }

  async getCodeBlockCount(): Promise<number> {
    const blocks = await this.getCodeBlocks();
    return blocks.length;
  }

  async getToolCards(): Promise<WebdriverIO.Element[]> {
    return $$(this.selectors.toolCard);
  }

  async getToolCardCount(): Promise<number> {
    const cards = await this.getToolCards();
    return cards.length;
  }

  async scrollToBottom(): Promise<void> {
    const scrollBtn = await this.isElementVisible(this.selectors.scrollToBottom);
    if (scrollBtn) {
      await this.safeClick(this.selectors.scrollToBottom);
    } else {
      await browser.execute((selector: string) => {
        const container = document.querySelector(selector);
        if (container) {
          container.scrollTop = container.scrollHeight;
        }
      }, this.selectors.container);
    }
  }

  async scrollToTop(): Promise<void> {
    await browser.execute((selector: string) => {
      const container = document.querySelector(selector);
      if (container) {
        container.scrollTop = 0;
      }
    }, this.selectors.container);
  }

  async isAtBottom(): Promise<boolean> {
    return browser.execute((selector: string) => {
      const container = document.querySelector(selector);
      if (!container) return true;
      const threshold = 50;
      return container.scrollHeight - container.scrollTop - container.clientHeight < threshold;
    }, this.selectors.container);
  }

  async getUserMessageTextAt(index: number): Promise<string> {
    const messages = await this.getUserMessages();
    if (index >= messages.length) {
      throw new Error(`User message index ${index} out of range (total: ${messages.length})`);
    }
    return messages[index].getText();
  }

  async getModelResponseTextAt(index: number): Promise<string> {
    const responses = await this.getModelResponses();
    if (index >= responses.length) {
      throw new Error(`Model response index ${index} out of range (total: ${responses.length})`);
    }
    return responses[index].getText();
  }
}

export default MessageList;

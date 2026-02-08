/**
 * Page object for chat view (workspace mode).
 */
import { BasePage } from './BasePage';
import { browser, $$ } from '@wdio/globals';
import { waitForStreamingComplete, waitForElementCountChange } from '../helpers/wait-utils';
import { environmentSettings } from '../config/capabilities';

export class ChatPage extends BasePage {
  private selectors = {
    appLayout: '[data-testid="app-layout"]',
    mainContent: '[data-testid="app-main-content"]',
    inputContainer: '[data-testid="chat-input-container"]',
    textarea: '[data-testid="chat-input-textarea"]',
    sendBtn: '[data-testid="chat-input-send-btn"]',
    messageList: '[data-testid="message-list"]',
    userMessage: '[data-testid^="user-message-"]',
    modelResponse: '[data-testid^="model-response-"]',
    modelSelector: '[data-testid="model-selector"]',
    modelDropdown: '[data-testid="model-selector-dropdown"]',
    toolCard: '[data-testid^="tool-card-"]',
    loadingIndicator: '[data-testid="loading-indicator"]',
    streamingIndicator: '[data-testid="streaming-indicator"]',
  };

  async waitForLoad(): Promise<void> {
    await this.waitForPageLoad();
    await this.waitForElement(this.selectors.appLayout);
    await this.wait(300);
  }

  async isChatInputVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.inputContainer);
  }

  async typeMessage(message: string): Promise<void> {
    await this.safeType(this.selectors.textarea, message);
  }

  async clickSend(): Promise<void> {
    await this.safeClick(this.selectors.sendBtn);
  }

  async sendMessage(message: string): Promise<void> {
    await this.typeMessage(message);
    await this.clickSend();
  }

  async sendMessageAndWaitResponse(
    message: string,
    timeout: number = environmentSettings.streamingResponseTimeout
  ): Promise<void> {
    const messagesBefore = await $$(this.selectors.modelResponse);
    const countBefore = messagesBefore.length;
    await this.sendMessage(message);
    await waitForElementCountChange(this.selectors.modelResponse, countBefore, timeout);
    await waitForStreamingComplete(this.selectors.modelResponse, 2000, timeout);
  }

  async getUserMessages(): Promise<string[]> {
    const messages = await $$(this.selectors.userMessage);
    const texts: string[] = [];
    
    for (const msg of messages) {
      const text = await msg.getText();
      texts.push(text);
    }
    
    return texts;
  }

  async getModelResponses(): Promise<string[]> {
    const responses = await $$(this.selectors.modelResponse);
    const texts: string[] = [];
    
    for (const resp of responses) {
      const text = await resp.getText();
      texts.push(text);
    }
    
    return texts;
  }

  async getLastModelResponse(): Promise<string> {
    const responses = await $$(this.selectors.modelResponse);
    
    if (responses.length === 0) {
      return '';
    }
    
    return responses[responses.length - 1].getText();
  }

  async getMessageCount(): Promise<{ user: number; model: number }> {
    const userMessages = await $$(this.selectors.userMessage);
    const modelResponses = await $$(this.selectors.modelResponse);
    
    return {
      user: userMessages.length,
      model: modelResponses.length,
    };
  }

  async isModelSelectorVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.modelSelector);
  }

  async clickModelSelector(): Promise<void> {
    await this.safeClick(this.selectors.modelSelector);
  }

  async waitForModelDropdown(): Promise<void> {
    await this.waitForElement(this.selectors.modelDropdown);
  }

  async getToolCardCount(): Promise<number> {
    const toolCards = await $$(this.selectors.toolCard);
    return toolCards.length;
  }

  async isLoading(): Promise<boolean> {
    return this.isElementVisible(this.selectors.loadingIndicator);
  }

  async isStreaming(): Promise<boolean> {
    return this.isElementVisible(this.selectors.streamingIndicator);
  }

  async waitForLoadingComplete(): Promise<void> {
    const isLoading = await this.isLoading();
    
    if (isLoading) {
      await browser.waitUntil(
        async () => !(await this.isLoading()),
        {
          timeout: environmentSettings.pageLoadTimeout,
          timeoutMsg: 'Loading did not complete',
        }
      );
    }
  }

  async clearInput(): Promise<void> {
    const element = await this.waitForElement(this.selectors.textarea);
    await element.clearValue();
  }

  async getInputValue(): Promise<string> {
    const element = await this.waitForElement(this.selectors.textarea);
    return element.getValue();
  }

  async isSendButtonEnabled(): Promise<boolean> {
    const element = await this.waitForElement(this.selectors.sendBtn);
    return element.isEnabled();
  }
}

export default ChatPage;

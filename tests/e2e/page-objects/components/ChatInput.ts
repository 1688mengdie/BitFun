/**
 * Page object for chat input (bottom message input area).
 */
import { BasePage } from '../BasePage';
import { browser } from '@wdio/globals';

export class ChatInput extends BasePage {
  private selectors = {
    container: '[data-testid="chat-input-container"]',
    textarea: '[data-testid="chat-input-textarea"]',
    sendBtn: '[data-testid="chat-input-send-btn"]',
    attachmentBtn: '[data-testid="chat-input-attachment-btn"]',
    cancelBtn: '[data-testid="chat-input-cancel-btn"]',
  };

  async isVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.container);
  }

  async waitForLoad(): Promise<void> {
    await this.waitForElement(this.selectors.container);
  }

  async typeMessage(message: string): Promise<void> {
    await this.safeType(this.selectors.textarea, message);
  }

  async getValue(): Promise<string> {
    const element = await this.waitForElement(this.selectors.textarea);
    return element.getValue();
  }

  async clear(): Promise<void> {
    const element = await this.waitForElement(this.selectors.textarea);
    await element.clearValue();
  }

  async clickSend(): Promise<void> {
    await this.safeClick(this.selectors.sendBtn);
  }

  async isSendButtonEnabled(): Promise<boolean> {
    const element = await this.waitForElement(this.selectors.sendBtn);
    return element.isEnabled();
  }

  async isSendButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.sendBtn);
  }

  async sendMessage(message: string): Promise<void> {
    await this.typeMessage(message);
    await this.clickSend();
  }

  async sendMessageWithEnter(message: string): Promise<void> {
    await this.typeMessage(message);
    await browser.keys(['Enter']);
  }

  async sendMessageWithCtrlEnter(message: string): Promise<void> {
    await this.typeMessage(message);
    await browser.keys(['Control', 'Enter']);
  }

  async clickAttachment(): Promise<void> {
    await this.safeClick(this.selectors.attachmentBtn);
  }

  async isAttachmentButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.attachmentBtn);
  }

  async clickCancel(): Promise<void> {
    await this.safeClick(this.selectors.cancelBtn);
  }

  async isCancelButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.cancelBtn);
  }

  async getPlaceholder(): Promise<string> {
    const element = await this.waitForElement(this.selectors.textarea);
    return element.getAttribute('placeholder') || '';
  }

  async focus(): Promise<void> {
    const element = await this.waitForElement(this.selectors.textarea);
    await element.click();
  }

  async isFocused(): Promise<boolean> {
    const element = await this.waitForElement(this.selectors.textarea);
    return element.isFocused();
  }
}

export default ChatInput;

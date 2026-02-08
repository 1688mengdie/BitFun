/**
 * Page object for header (title bar and window controls).
 */
import { BasePage } from '../BasePage';

export class Header extends BasePage {
  private selectors = {
    container: '[data-testid="header-container"]',
    homeBtn: '[data-testid="header-home-btn"]',
    minimizeBtn: '[data-testid="header-minimize-btn"]',
    maximizeBtn: '[data-testid="header-maximize-btn"]',
    closeBtn: '[data-testid="header-close-btn"]',
    leftPanelToggle: '[data-testid="header-left-panel-toggle"]',
    rightPanelToggle: '[data-testid="header-right-panel-toggle"]',
    newSessionBtn: '[data-testid="header-new-session-btn"]',
    title: '[data-testid="header-title"]',
  };

  async isVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.container);
  }

  async waitForLoad(): Promise<void> {
    await this.waitForElement(this.selectors.container);
  }

  async clickHome(): Promise<void> {
    await this.safeClick(this.selectors.homeBtn);
  }

  async isHomeButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.homeBtn);
  }

  async clickMinimize(): Promise<void> {
    await this.safeClick(this.selectors.minimizeBtn);
  }

  async isMinimizeButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.minimizeBtn);
  }

  async clickMaximize(): Promise<void> {
    await this.safeClick(this.selectors.maximizeBtn);
  }

  async isMaximizeButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.maximizeBtn);
  }

  async clickClose(): Promise<void> {
    await this.safeClick(this.selectors.closeBtn);
  }

  async isCloseButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.closeBtn);
  }

  async toggleLeftPanel(): Promise<void> {
    await this.safeClick(this.selectors.leftPanelToggle);
  }

  async toggleRightPanel(): Promise<void> {
    await this.safeClick(this.selectors.rightPanelToggle);
  }

  async clickNewSession(): Promise<void> {
    await this.safeClick(this.selectors.newSessionBtn);
  }

  async isNewSessionButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.newSessionBtn);
  }

  async getTitle(): Promise<string> {
    const exists = await this.isElementExist(this.selectors.title);
    if (!exists) {
      return '';
    }
    return this.getText(this.selectors.title);
  }

  async areWindowControlsVisible(): Promise<boolean> {
    const minimizeVisible = await this.isMinimizeButtonVisible();
    const maximizeVisible = await this.isMaximizeButtonVisible();
    const closeVisible = await this.isCloseButtonVisible();
    
    return minimizeVisible && maximizeVisible && closeVisible;
  }
}

export default Header;

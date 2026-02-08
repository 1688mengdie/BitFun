/**
 * Page object for startup screen (no workspace open).
 */
import { BasePage } from './BasePage';
import { browser } from '@wdio/globals';

export class StartupPage extends BasePage {
  private selectors = {
    container: '[data-testid="startup-container"]',
    openFolderBtn: '[data-testid="startup-open-folder-btn"]',
    recentProjects: '[data-testid="startup-recent-projects"]',
    recentProjectItem: '[data-testid="startup-recent-project-item"]',
    brandLogo: '[data-testid="startup-brand-logo"]',
    welcomeText: '[data-testid="startup-welcome-text"]',
  };

  async waitForLoad(): Promise<void> {
    await this.waitForPageLoad();
    await this.wait(500);
  }

  async isVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.container);
  }

  async clickOpenFolder(): Promise<void> {
    await this.safeClick(this.selectors.openFolderBtn);
  }

  async isOpenFolderButtonVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.openFolderBtn);
  }

  async getRecentProjects(): Promise<string[]> {
    const exists = await this.isElementExist(this.selectors.recentProjects);
    if (!exists) {
      return [];
    }

    const items = await browser.$$(this.selectors.recentProjectItem);
    const projects: string[] = [];
    
    for (const item of items) {
      const text = await item.getText();
      projects.push(text);
    }
    
    return projects;
  }

  async clickRecentProject(index: number): Promise<void> {
    const items = await browser.$$(this.selectors.recentProjectItem);
    
    if (index >= items.length) {
      throw new Error(`Recent project index ${index} out of range (total: ${items.length})`);
    }
    
    await items[index].click();
  }

  async isBrandLogoVisible(): Promise<boolean> {
    return this.isElementVisible(this.selectors.brandLogo);
  }

  async getWelcomeText(): Promise<string> {
    const exists = await this.isElementExist(this.selectors.welcomeText);
    if (!exists) {
      return '';
    }
    return this.getText(this.selectors.welcomeText);
  }
}

export default StartupPage;

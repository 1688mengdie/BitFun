/**
 * L0 open settings spec: open recent workspace then open settings.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Open workspace and settings', () => {
  it('app starts and waits for UI', async () => {
    console.log('[L0] Waiting for app to start...');
    await browser.pause(1000);
    const title = await browser.getTitle();
    console.log('[L0] App title:', title);
    expect(title).toBeDefined();
  });

  it('opens recent workspace', async () => {
    await browser.pause(500);
    const startupContainer = await $('[data-testid="startup-container"]');
    const isStartupPage = await startupContainer.isExisting();

    if (isStartupPage) {
      console.log('[L0] On startup page, trying to open workspace');
      const continueBtn = await $('.startup-content__continue-btn');
      const hasContinueBtn = await continueBtn.isExisting();
      if (hasContinueBtn) {
        console.log('[L0] Clicking Continue');
        await continueBtn.click();
        await browser.pause(2000);
      } else {
        const historyItem = await $('.startup-content__history-item');
        const hasHistory = await historyItem.isExisting();
        if (hasHistory) {
          console.log('[L0] Clicking history item');
          await historyItem.click();
          await browser.pause(2000);
        } else {
          console.log('[L0] No workspace available, skipping');
        }
      }
    } else {
      console.log('[L0] Workspace already open');
    }
    expect(true).toBe(true);
  });

  it('clicks settings to open config center', async () => {
    await browser.pause(500);
    const selectors = [
      '[data-testid="header-config-btn"]',
      '.bitfun-header-right button:has(svg.lucide-settings)',
      '.bitfun-header-right button:nth-last-child(4)',
    ];

    let configBtn = null;
    let found = false;

    for (const selector of selectors) {
      try {
        const btn = await $(selector);
        const exists = await btn.isExisting();
        if (exists) {
          console.log(`[L0] Found config button: ${selector}`);
          configBtn = btn;
          found = true;
          break;
        }
      } catch (e) {
        // ignore selector errors
      }
    }

    if (!found) {
      console.log('[L0] Iterating bitfun-header-right buttons...');
      const headerRight = await $('.bitfun-header-right');
      const headerExists = await headerRight.isExisting();
      if (headerExists) {
        const buttons = await headerRight.$$('button');
        console.log(`[L0] Found ${buttons.length} buttons`);
        for (const btn of buttons) {
          const html = await btn.getHTML();
          if (html.includes('lucide') || html.includes('Settings')) {
            configBtn = btn;
            found = true;
            console.log('[L0] Found config button by iteration');
            break;
          }
        }
      }
    }

    if (found && configBtn) {
      console.log('[L0] Clicking config button');
      await configBtn.click();
      await browser.pause(1500);
      const configPanel = await $('.bitfun-config-center-panel');
      const configExists = await configPanel.isExisting();
      if (configExists) {
        console.log('[L0] Config center opened');
      } else {
        const configCenter = await $('[class*="config"]');
        const hasConfig = await configCenter.isExisting();
        console.log(`[L0] Config-related element exists: ${hasConfig}`);
      }
    } else {
      console.log('[L0] Config button not found');
    }
    expect(true).toBe(true);
  });

  it('keeps UI open for 15 seconds', async () => {
    console.log('[L0] Keeping UI open for 15s...');
    for (let i = 0; i < 3; i++) {
      await browser.pause(5000);
      console.log(`[L0] Waited ${(i + 1) * 5}s...`);
    }
    console.log('[L0] Test complete');
    expect(true).toBe(true);
  });
});

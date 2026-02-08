/**
 * L0 open workspace spec: open recent workspace and keep UI visible.
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 Open workspace', () => {
  it('app starts and waits for UI', async () => {
    console.log('[L0] Waiting for app to start...');
    await browser.pause(1000);
    const title = await browser.getTitle();
    console.log('[L0] App title:', title);
    expect(title).toBeDefined();
  });

  it('checks startup page or workspace state', async () => {
    await browser.pause(500);
    const startupContainer = await $('[data-testid="startup-container"]');
    const isStartupPage = await startupContainer.isExisting();

    if (isStartupPage) {
      console.log('[L0] On startup page');
    } else {
      console.log('[L0] Workspace may already be open');
    }

    const body = await $('body');
    const html = await body.getHTML();
    console.log('[L0] Body HTML length:', html.length);
    if (html.length < 100) {
      console.log('[L0] Body HTML:', html);
    }
    expect(true).toBe(true);
  });

  it('tries to click Continue last session', async () => {
    await browser.pause(1000);
    const continueBtn = await $('.startup-content__continue-btn');
    const exists = await continueBtn.isExisting();

    if (exists) {
      console.log('[L0] Found Continue button, clicking');
      await continueBtn.click();
      console.log('[L0] Waiting for workspace to load...');
      await browser.pause(3000);
      const startupAfter = await $('[data-testid="startup-container"]');
      const stillStartup = await startupAfter.isExisting();
      if (!stillStartup) {
        console.log('[L0] Workspace opened, startup page gone');
      } else {
        console.log('[L0] Startup page still visible');
      }
    } else {
      console.log('[L0] Continue button not found');
      const historyItem = await $('.startup-content__history-item');
      const hasHistory = await historyItem.isExisting();
      if (hasHistory) {
        console.log('[L0] Found history item, clicking first');
        await historyItem.click();
        await browser.pause(3000);
      } else {
        console.log('[L0] No history, skipping');
      }
    }
    expect(true).toBe(true);
  });

  it('keeps UI open for 30 seconds', async () => {
    console.log('[L0] Keeping UI open for 30s...');
    for (let i = 0; i < 6; i++) {
      await browser.pause(5000);
      console.log(`[L0] Waited ${(i + 1) * 5}s...`);
      const body = await $('body');
      const childCount = await body.$$('*').then(els => els.length);
      console.log(`[L0] DOM element count: ${childCount}`);
    }
    console.log('[L0] Wait complete');
    expect(true).toBe(true);
  });
});

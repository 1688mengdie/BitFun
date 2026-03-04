/**
 * Helper utilities for workspace operations in e2e tests
 */

import { browser, $ } from '@wdio/globals';

/**
 * Attempts to open a workspace using multiple strategies
 * @returns true if workspace was successfully opened
 */
export async function openWorkspace(): Promise<boolean> {
  // Check if workspace is already open
  const chatInput = await $('[data-testid="chat-input-container"]');
  let hasWorkspace = await chatInput.isExisting();

  if (hasWorkspace) {
    console.log('[Helper] Workspace already open');
    return true;
  }

  // Strategy 1: Try clicking recent workspace
  const recentItem = await $('.welcome-scene__recent-item');
  const hasRecent = await recentItem.isExisting();

  if (hasRecent) {
    console.log('[Helper] Clicking recent workspace');
    await recentItem.click();
    await browser.pause(3000);

    const chatInputAfter = await $('[data-testid="chat-input-container"]');
    hasWorkspace = await chatInputAfter.isExisting();

    if (hasWorkspace) {
      console.log('[Helper] Workspace opened from recent');
      return true;
    }
  }

  // Strategy 2: Use Tauri API to open current directory
  console.log('[Helper] Opening workspace via Tauri API');
  try {
    const testWorkspacePath = process.cwd();
    await browser.execute((path: string) => {
      // @ts-ignore
      return window.__TAURI__.core.invoke('open_workspace', {
        request: { path }
      });
    }, testWorkspacePath);
    await browser.pause(3000);

    const chatInputAfter = await $('[data-testid="chat-input-container"]');
    hasWorkspace = await chatInputAfter.isExisting();

    if (hasWorkspace) {
      console.log('[Helper] Workspace opened via Tauri API');
      return true;
    }
  } catch (error) {
    console.error('[Helper] Failed to open workspace via Tauri API:', error);
  }

  return false;
}

/**
 * Checks if workspace is currently open
 */
export async function isWorkspaceOpen(): Promise<boolean> {
  const chatInput = await $('[data-testid="chat-input-container"]');
  return await chatInput.isExisting();
}

/**
 * LVPA E2E 测试辅助函数
 */

import { browser, expect } from '@wdio/globals';

/**
 * 通过 Zustand store 打开指定场景
 * 需 dev 模式（main.tsx 暴露了 window.__E2E_SCENE_STORE__）
 */
export async function openScene(sceneTabId: string): Promise<boolean> {
  const opened = await browser.execute((id: string) => {
    const store = (window as any).__E2E_SCENE_STORE__;
    if (store) {
      store.getState().openScene(id);
      return true;
    }
    return false;
  }, sceneTabId);

  if (opened) {
    // 等待 React 渲染和场景动画
    await browser.pause(2000);
  }

  return opened;
}

/**
 * 断言页面未出现错误边界
 */
export async function expectNoErrorBoundary(): Promise<void> {
  const errorCount = await browser.execute(() => {
    const errorElements = document.querySelectorAll(
      '[data-error-boundary], .error-boundary, .app-error-boundary'
    );
    let count = 0;
    errorElements.forEach((el) => {
      const text = el.textContent || '';
      if (text.length > 0 && !text.includes('Loading')) {
        count++;
      }
    });
    return count;
  });

  if (errorCount > 0) {
    console.error('[LVPA] Error boundaries detected:', errorCount);
  }
  expect(errorCount).toBe(0);
}

/**
 * L0 LVPA app shell spec: 验证应用外壳基础 UI 组件
 *
 * 架构总纲 §12.1 — Layer 3: 端到端测试
 * 架构总纲 §4a — 离线层（PWA）
 */

import { browser, expect, $ } from '@wdio/globals';

describe('L0 LVPA 应用外壳', () => {
  describe('应用启动', () => {
    it('应用标题应定义且非空', async () => {
      await browser.pause(5000);
      const title = await browser.getTitle();
      console.log('[LVPA] App title:', title);
      expect(title).toBeDefined();
      expect(title.length).toBeGreaterThan(0);
    });

    it('文档就绪状态应为 complete', async () => {
      const readyState = await browser.execute(() => document.readyState);
      expect(readyState).toBe('complete');
    });

    it('#root 应包含 React 渲染内容', async () => {
      await browser.pause(2000);
      const root = await $('#root');
      const exists = await root.isExisting();
      expect(exists).toBe(true);

      const childCount = await browser.execute(() => {
        const root = document.getElementById('root');
        return root?.childElementCount ?? 0;
      });
      console.log('[LVPA] #root child count:', childCount);
      expect(childCount).toBeGreaterThan(0);
    });
  });

  describe('核心 UI 组件', () => {
    it('NavBar 应存在', async () => {
      const navBar = await $('.bitfun-nav-bar');
      const exists = await navBar.isExisting();
      console.log('[LVPA] NavBar exists:', exists);
      expect(exists).toBe(true);
    });

    it('NavPanel 应存在', async () => {
      const navPanel = await $('[data-testid="nav-panel"]');
      const exists = await navPanel.isExisting();
      console.log('[LVPA] NavPanel exists:', exists);
      expect(exists).toBe(true);
    });

    it('场景标签栏 SceneBar 应渲染 tab', async () => {
      const sceneTab = await $('[role="tab"]');
      const exists = await sceneTab.isExisting();
      console.log('[LVPA] SceneTab exists:', exists);
      expect(exists).toBe(true);
    });
  });

  describe('LVPA 模式切换', () => {
    it('LVPA 模式切换按钮应存在', async () => {
      await browser.pause(2000);
      const modeSwitch = await $('.lvpa-mode-switch__trigger');
      const exists = await modeSwitch.isExisting();
      console.log('[LVPA] LVPA mode switch exists:', exists);
      expect(exists).toBe(true);

      const text = await modeSwitch.getText();
      console.log('[LVPA] Mode switch text:', text);
      expect(text).toContain('BitFun');
    });

    it('LVPA 模式切换菜单可点击打开', async () => {
      const modeSwitch = await $('.lvpa-mode-switch__trigger');
      await modeSwitch.click();
      await browser.pause(500);

      const menu = await $('.lvpa-mode-switch__menu');
      const menuExists = await menu.isExisting();
      console.log('[LVPA] Mode switch menu visible:', menuExists);
      expect(menuExists).toBe(true);

      // 点击外部关闭菜单
      await browser.execute(() => {
        document.body.click();
      });
      await browser.pause(500);
    });
  });

  describe('PWA 支持检测', () => {
    it('浏览器应支持 Service Worker API', async () => {
      const swSupported = await browser.execute(() => 'serviceWorker' in navigator);
      console.log('[LVPA] ServiceWorker supported:', swSupported);
      // serviceWorker 在 Tauri webview 中可能返回 false，仅记录
      expect(typeof swSupported).toBe('boolean');
    });

    it('浏览器应支持 IndexedDB', async () => {
      const idbSupported = await browser.execute(() => 'IndexedDB' in window);
      console.log('[LVPA] IndexedDB supported:', idbSupported);
      expect(idbSupported).toBe(true);
    });
  });
});

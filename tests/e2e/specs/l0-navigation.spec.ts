/**
 * L0 navigation spec: verifies sidebar navigation panel exists and items are visible.
 * Basic checks that navigation structure is present - no AI interaction needed.
 */

import { browser, expect, $ } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';

describe('L0 Navigation Panel', () => {
  let hasWorkspace = false;

  describe('Navigation panel existence', () => {
    it('app should start successfully', async () => {
      console.log('[L0] Starting navigation tests...');
      await browser.pause(3000);
      const title = await browser.getTitle();
      console.log('[L0] App title:', title);
      expect(title).toBeDefined();
    });

    it('should detect workspace or startup state', async () => {
      await browser.pause(1000);

      hasWorkspace = await openWorkspace();

      console.log('[L0] Workspace opened:', hasWorkspace);
      expect(hasWorkspace).toBe(true);
    });

    it('should have navigation panel or sidebar when workspace is open', async function () {
      expect(hasWorkspace).toBe(true);

      await browser.pause(1000);

      const selectors = [
        '[data-testid="nav-panel"]',
        '.bitfun-nav-panel',
        '[class*="nav-panel"]',
        '[class*="NavPanel"]',
        'nav',
        '.sidebar',
      ];

      let navFound = false;
      for (const selector of selectors) {
        const element = await $(selector);
        const exists = await element.isExisting();

        if (exists) {
          console.log(`[L0] Navigation panel found: ${selector}`);
          navFound = true;
          break;
        }
      }

      expect(navFound).toBe(true);
    });
  });

  describe('Navigation items visibility', () => {
    it('navigation items should be present if workspace is open', async function () {
      expect(hasWorkspace).toBe(true);

      await browser.pause(500);
      
      const navItemSelectors = [
        '.bitfun-nav-panel__item',
        '[data-testid^="nav-item-"]',
        '[class*="nav-item"]',
        '.nav-item',
        '.bitfun-nav-panel__inline-item',
      ];

      let itemsFound = false;
      let itemCount = 0;

      for (const selector of navItemSelectors) {
        try {
          const items = await browser.$$(selector);
          if (items.length > 0) {
            console.log(`[L0] Found ${items.length} navigation items: ${selector}`);
            itemsFound = true;
            itemCount = items.length;
            break;
          }
        } catch (e) {
          // Continue to next selector
        }
      }

      expect(itemsFound).toBe(true);
      expect(itemCount).toBeGreaterThan(0);
    });

    it('navigation sections should be present', async function () {
      expect(hasWorkspace).toBe(true);

      const sectionSelectors = [
        '.bitfun-nav-panel__sections',
        '.bitfun-nav-panel__section-label',
        '[class*="nav-section"]',
        '.nav-section',
      ];

      let sectionsFound = false;
      for (const selector of sectionSelectors) {
        const sections = await browser.$$(selector);
        if (sections.length > 0) {
          console.log(`[L0] Found ${sections.length} navigation sections: ${selector}`);
          sectionsFound = true;
          break;
        }
      }

      if (!sectionsFound) {
        console.log('[L0] Navigation sections not found (may use different structure)');
      }

      expect(sectionsFound).toBe(true);
    });
  });

  describe('Navigation interactivity', () => {
    it('navigation items should be clickable', async function () {
      expect(hasWorkspace).toBe(true);

      const navItems = await browser.$$('.bitfun-nav-panel__inline-item');
      
      if (navItems.length === 0) {
        const altItems = await browser.$$('.bitfun-nav-panel__item');
        expect(altItems.length).toBeGreaterThan(0);
      }

      const firstItem = navItems.length > 0 ? navItems[0] : (await browser.$$('.bitfun-nav-panel__item'))[0];
      const isClickable = await firstItem.isClickable();
      console.log('[L0] First nav item clickable:', isClickable);

      expect(isClickable).toBe(true);
    });
  });

  after(async () => {
    console.log('[L0] Navigation tests complete');
  });
});

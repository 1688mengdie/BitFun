/**
 * Wait utilities for E2E (element stable, streaming, loading, etc.).
 */
import { browser, $, $$ } from '@wdio/globals';
import { environmentSettings } from '../config/capabilities';

export async function waitForElementStable(
  selector: string,
  stableTime: number = 500,
  timeout: number = environmentSettings.defaultTimeout
): Promise<void> {
  const startTime = Date.now();
  let lastRect: { x: number; y: number; width: number; height: number } | null = null;
  let stableStartTime: number | null = null;

  await browser.waitUntil(
    async () => {
      const element = await $(selector);
      if (!(await element.isDisplayed())) {
        return false;
      }

      const rect = await element.getLocation();
      const size = await element.getSize();
      const currentRect = {
        x: rect.x,
        y: rect.y,
        width: size.width,
        height: size.height,
      };

      if (lastRect && 
          lastRect.x === currentRect.x &&
          lastRect.y === currentRect.y &&
          lastRect.width === currentRect.width &&
          lastRect.height === currentRect.height) {
        if (!stableStartTime) {
          stableStartTime = Date.now();
        }
        return Date.now() - stableStartTime >= stableTime;
      } else {
        lastRect = currentRect;
        stableStartTime = null;
        return false;
      }
    },
    {
      timeout,
      timeoutMsg: `Element ${selector} did not stabilize within ${timeout}ms`,
      interval: 100,
    }
  );
}

export async function waitForStreamingComplete(
  messageSelector: string,
  stableTime: number = 2000,
  timeout: number = environmentSettings.streamingResponseTimeout
): Promise<void> {
  let lastContent = '';
  let stableStartTime: number | null = null;

  await browser.waitUntil(
    async () => {
      const messages = await $$(messageSelector);
      if (messages.length === 0) {
        return false;
      }

      const lastMessage = messages[messages.length - 1];
      const currentContent = await lastMessage.getText();

      if (currentContent === lastContent && currentContent.length > 0) {
        if (!stableStartTime) {
          stableStartTime = Date.now();
        }
        return Date.now() - stableStartTime >= stableTime;
      } else {
        lastContent = currentContent;
        stableStartTime = null;
        return false;
      }
    },
    {
      timeout,
      timeoutMsg: `Streaming response did not complete within ${timeout}ms`,
      interval: 500,
    }
  );
}

export async function waitForAnimationEnd(
  selector: string,
  timeout: number = environmentSettings.animationTimeout
): Promise<void> {
  const element = await $(selector);
  
  await browser.waitUntil(
    async () => {
      const animationState = await browser.execute((sel: string) => {
        const el = document.querySelector(sel);
        if (!el) return true;
        
        const computedStyle = window.getComputedStyle(el);
        const animationName = computedStyle.animationName;
        const transitionDuration = parseFloat(computedStyle.transitionDuration);
        
        return animationName === 'none' && transitionDuration === 0;
      }, selector);
      
      return animationState;
    },
    {
      timeout,
      timeoutMsg: `Animation on ${selector} did not complete within ${timeout}ms`,
      interval: 100,
    }
  );
}

export async function waitForLoadingComplete(
  loadingSelector: string = '[data-testid="loading-indicator"]',
  timeout: number = environmentSettings.pageLoadTimeout
): Promise<void> {
  const element = await $(loadingSelector);
  const exists = await element.isExisting();
  if (exists) {
    await element.waitForDisplayed({
      timeout,
      reverse: true,
      timeoutMsg: `Loading indicator did not disappear within ${timeout}ms`,
    });
  }
}

export async function waitForElementCountChange(
  selector: string,
  initialCount: number,
  timeout: number = environmentSettings.defaultTimeout
): Promise<number> {
  let newCount = initialCount;

  await browser.waitUntil(
    async () => {
      const elements = await $$(selector);
      newCount = elements.length;
      return newCount !== initialCount;
    },
    {
      timeout,
      timeoutMsg: `Element count for ${selector} did not change from ${initialCount} within ${timeout}ms`,
      interval: 200,
    }
  );

  return newCount;
}

export async function waitForTextPresent(
  text: string,
  timeout: number = environmentSettings.defaultTimeout
): Promise<void> {
  await browser.waitUntil(
    async () => {
      const pageText = await browser.execute(() => document.body.innerText);
      return pageText.includes(text);
    },
    {
      timeout,
      timeoutMsg: `Text "${text}" did not appear within ${timeout}ms`,
      interval: 200,
    }
  );
}

export async function waitForAttributeChange(
  selector: string,
  attribute: string,
  expectedValue: string,
  timeout: number = environmentSettings.defaultTimeout
): Promise<void> {
  await browser.waitUntil(
    async () => {
      const element = await $(selector);
      const value = await element.getAttribute(attribute);
      return value === expectedValue;
    },
    {
      timeout,
      timeoutMsg: `Attribute ${attribute} of ${selector} did not become "${expectedValue}" within ${timeout}ms`,
      interval: 200,
    }
  );
}

export async function waitForNetworkIdle(
  idleTime: number = 1000,
  _timeout: number = environmentSettings.defaultTimeout
): Promise<void> {
  await browser.pause(idleTime);
}

export default {
  waitForElementStable,
  waitForStreamingComplete,
  waitForAnimationEnd,
  waitForLoadingComplete,
  waitForElementCountChange,
  waitForTextPresent,
  waitForAttributeChange,
  waitForNetworkIdle,
};

import React, { createContext, useCallback, useEffect, useState } from 'react';
import { darkTheme } from './presets/dark';
import { lightTheme } from './presets/light';

export type ThemeId = 'dark' | 'light';

interface ThemeContextValue {
  themeId: ThemeId;
  isDark: boolean;
  setTheme: (id: ThemeId) => void;
  toggleTheme: () => void;
}

const STORAGE_KEY = 'bitfun-mobile-theme';
const THEME_STYLE_ID = 'bitfun-theme-vars';

const themeMap: Record<ThemeId, Record<string, string>> = {
  dark: darkTheme,
  light: lightTheme,
};

const themeColors: Record<ThemeId, string> = {
  dark: '#121214',
  light: '#f2f2f7',
};

const textColors: Record<ThemeId, string> = {
  dark: '#e8e8e8',
  light: '#1c1c1e',
};

let fadeTimer: ReturnType<typeof setTimeout> | undefined;
let fadeRaf: number | undefined;

/**
 * Build a complete CSS stylesheet string that defines all theme variables
 * on :root plus explicit background/color on html and body.
 * Injecting a <style> element is far more reliable across mobile WebKit
 * than calling setProperty() on each variable individually.
 */
function buildThemeCSS(id: ThemeId): string {
  const vars = themeMap[id];
  const bg = themeColors[id];
  const fg = textColors[id];
  const parts: string[] = [':root {'];

  for (const [key, value] of Object.entries(vars)) {
    parts.push(`  ${key}: ${value};`);
  }
  parts.push(`  color-scheme: ${id};`);
  parts.push('}');
  parts.push(`html, body { background-color: ${bg}; color: ${fg}; color-scheme: ${id}; }`);

  return parts.join('\n');
}

function applyTheme(id: ThemeId) {
  const root = document.documentElement;
  const body = document.body;
  const prevTheme = root.getAttribute('data-theme');
  const isSwitch = prevTheme && prevTheme !== id;

  // --- 1. Inject / update <style> with all CSS variables ---
  let styleEl = document.getElementById(THEME_STYLE_ID) as HTMLStyleElement | null;
  if (!styleEl) {
    styleEl = document.createElement('style');
    styleEl.id = THEME_STYLE_ID;
    document.head.appendChild(styleEl);
  }
  styleEl.textContent = buildThemeCSS(id);

  // --- 2. data-theme attribute (for any CSS selectors that depend on it) ---
  root.setAttribute('data-theme', id);
  root.setAttribute('data-theme-type', id);
  body.setAttribute('data-theme', id);

  // --- 3. Inline style fallbacks (highest specificity, belt-and-suspenders) ---
  body.style.backgroundColor = themeColors[id];
  body.style.color = textColors[id];

  // --- 4. Update <meta name="theme-color"> ---
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) {
    meta.setAttribute('content', themeColors[id]);
    meta.removeAttribute('media');
  }

  // --- 5. Visual crossfade on switch (opacity only, most compatible) ---
  if (isSwitch) {
    if (fadeRaf != null) cancelAnimationFrame(fadeRaf);
    clearTimeout(fadeTimer);

    const app = document.querySelector('.mobile-app') as HTMLElement | null;
    if (app) {
      app.style.transition = 'none';
      app.style.opacity = '0.55';

      fadeRaf = requestAnimationFrame(() => {
        fadeRaf = requestAnimationFrame(() => {
          app.style.transition = 'opacity 0.28s ease-out';
          app.style.opacity = '1';
          fadeTimer = setTimeout(() => {
            app.style.removeProperty('transition');
            app.style.removeProperty('opacity');
          }, 320);
        });
      });
    }
  }
}

function getInitialTheme(): ThemeId {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'dark' || stored === 'light') return stored;
  } catch { /* ignore */ }
  return window.matchMedia?.('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

export const ThemeContext = createContext<ThemeContextValue>({
  themeId: 'dark',
  isDark: true,
  setTheme: () => {},
  toggleTheme: () => {},
});

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [themeId, setThemeId] = useState<ThemeId>(getInitialTheme);

  useEffect(() => {
    applyTheme(themeId);
    try { localStorage.setItem(STORAGE_KEY, themeId); } catch { /* ignore */ }
  }, [themeId]);

  const setTheme = useCallback((id: ThemeId) => setThemeId(id), []);
  const toggleTheme = useCallback(() => setThemeId(prev => prev === 'dark' ? 'light' : 'dark'), []);

  return (
    <ThemeContext.Provider value={{ themeId, isDark: themeId === 'dark', setTheme, toggleTheme }}>
      {children}
    </ThemeContext.Provider>
  );
};

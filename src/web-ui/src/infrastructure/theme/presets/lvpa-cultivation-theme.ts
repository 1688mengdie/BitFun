// LVPA 修仙亮色主题 — 太初宗·昼
//
// 基于 china-style-theme 扩展，覆盖 colors 部分追加 LVPA 五色系统。
// 架构源：量价时空/四总纲/架构总纲.md §4a
// 色值定义：量价时空/Phase-6-类型契约.md §三

import { ThemeConfig } from '../types';
import {
  createAccentScale,
  createChinaTypography,
  createCompactRadius,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  createStandardEasing,
  createStandardSpacing,
  rgbFromHex,
  rgbaFromHex,
  STATIC_BLACK,
  STATIC_WHITE,
} from './shared';
import {
  LVPA_COLOR_TOKENS,
} from './lvpa-design-tokens';

// ── LVPA 五色常量 ──
const LVPA_INK = LVPA_COLOR_TOKENS.ink;          // #1c1c1f
const LVPA_VERMILLION = LVPA_COLOR_TOKENS.vermillion; // #c8102e
const LVPA_GOLD = LVPA_COLOR_TOKENS.gold;          // #d4a843
const LVPA_JADE = LVPA_COLOR_TOKENS.jade;          // #7eb09b
const LVPA_CLOUD = LVPA_COLOR_TOKENS.cloud;        // #faf8f0

// ── 派生常量 ──
const LVPA_ACCENT = LVPA_JADE;
const LVPA_ACCENT_HOVER = '#5a9078';
const LVPA_SECONDARY = LVPA_GOLD;
const LVPA_SECONDARY_HOVER = '#b8922e';
const LVPA_BUTTON_TEXT = '#3d3d3d';
const LVPA_SUCCESS = '#52ad5a';
const LVPA_WARNING = LVPA_GOLD;
const LVPA_ERROR = LVPA_VERMILLION;
const LVPA_INFO = LVPA_JADE;
const LVPA_BORDER = '#5a6b50';

const lvpaAccent = (alpha: number | string) => rgbaFromHex(LVPA_ACCENT, alpha);
const lvpaBorder = (alpha: number | string) => rgbaFromHex(LVPA_BORDER, alpha);

export const lvpaCultivationTheme: ThemeConfig = {
  id: 'lvpa-cultivation',
  name: '太初宗·昼',
  type: 'light',
  description: 'LVPA 修仙亮色主题 — 宣纸朱砂，金线青玉，如日之升',
  author: 'LVPA Team',
  version: '1.0.0',

  // ── Color System ──
  colors: {
    background: {
      primary: LVPA_CLOUD,
      secondary: '#f5f3e8',
      tertiary: '#f0ede0',
      elevated: '#f0ede0',
      workbench: LVPA_CLOUD,
      scene: LVPA_CLOUD,
    },

    text: {
      primary: LVPA_INK,
      secondary: '#3d3d3d',
      muted: '#6a6a6a',
      disabled: '#9a9a9a',
    },

    // 主强调色从 china-style 的蓝（#2e5e8a）→ 青玉（#7eb09b）
    accent: createAccentScale({ base: LVPA_ACCENT, hover: LVPA_ACCENT_HOVER }),

    // 副强调色从 china-style 的绿（#7eb09b）→ 金线（#d4a843）
    purple: createSecondaryAccentScale({ base: LVPA_SECONDARY, hover: LVPA_SECONDARY_HOVER }),

    semantic: createSemanticColors({
      success: LVPA_SUCCESS,
      warning: LVPA_WARNING,
      error: LVPA_ERROR,
      info: LVPA_INFO,
      bgAlpha: 0.08,
      borderAlpha: 0.25,
    }),

    border: {
      subtle: lvpaBorder(0.12),
      base: lvpaBorder(0.2),
      medium: lvpaBorder(0.28),
      strong: lvpaBorder(0.36),
      prominent: lvpaBorder(0.48),
    },

    element: {
      subtle: lvpaAccent(0.03),
      soft: lvpaAccent(0.06),
      base: lvpaAccent(0.1),
      medium: lvpaAccent(0.14),
      strong: lvpaAccent(0.18),
    },

    git: createGitColors({
      branch: rgbFromHex(LVPA_ACCENT),
      branchBg: lvpaAccent(0.08),
      changes: rgbFromHex(LVPA_WARNING),
      added: rgbFromHex(LVPA_SUCCESS),
      deleted: rgbFromHex(LVPA_ERROR),
    }),
  },

  // ── Effects ──
  effects: {
    shadow: {
      xs: `0 1px 2px ${lvpaBorder(0.06)}`,
      sm: `0 2px 4px ${lvpaBorder(0.08)}`,
      base: `0 4px 8px ${lvpaBorder(0.1)}`,
      lg: `0 8px 16px ${lvpaBorder(0.12)}`,
      xl: `0 12px 24px ${lvpaBorder(0.15)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.03)',
      base: 'blur(8px) saturate(1.05)',
    },

    radius: createCompactRadius(),
    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.5,
      hover: 0.75,
      focus: 0.9,
    },
  },

  // ── Motion ──
  motion: {
    duration: {
      instant: '0.1s',
      fast: '0.2s',
      base: '0.35s',
      slow: '0.7s',
    },
    easing: createStandardEasing('cubic-bezier(0.25, 0.1, 0.25, 1)'),
  },

  // ── Typography ──
  typography: createChinaTypography(),

  // ── Components ──
  components: {
    button: {
      primary: {
        default: {
          background: STATIC_BLACK,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: '#262626',
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: LVPA_INK,
          color: STATIC_WHITE,
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      ghost: {
        default: {
          color: '#555555',
        },
        hover: {
          background: lvpaAccent(0.11),
          color: LVPA_BUTTON_TEXT,
          border: 'transparent',
        },
      },
    },
  },

  // ── Monaco Editor ──
  monaco: {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '6a6a6a', fontStyle: 'italic' },
      { token: 'keyword', foreground: LVPA_VERMILLION },
      { token: 'string', foreground: LVPA_SUCCESS },
      { token: 'number', foreground: LVPA_GOLD },
      { token: 'type', foreground: LVPA_JADE },
      { token: 'class', foreground: LVPA_JADE },
      { token: 'function', foreground: LVPA_JADE },
      { token: 'variable', foreground: '3d3d3d' },
      { token: 'constant', foreground: 'a0522d' },
      { token: 'operator', foreground: LVPA_VERMILLION },
      { token: 'tag', foreground: LVPA_JADE },
      { token: 'attribute.name', foreground: LVPA_JADE },
      { token: 'attribute.value', foreground: LVPA_SUCCESS },
    ],
    colors: {
      background: LVPA_CLOUD,
      foreground: LVPA_INK,
      lineHighlight: '#f5f3e8',
      selection: lvpaAccent(0.28),
      cursor: LVPA_ACCENT,
      'editor.selectionBackground': lvpaAccent(0.28),
      'editor.selectionForeground': LVPA_INK,
      'editor.inactiveSelectionBackground': lvpaAccent(0.18),
      'editor.selectionHighlightBackground': lvpaAccent(0.2),
      'editor.selectionHighlightBorder': lvpaAccent(0.35),
      'editorCursor.foreground': LVPA_ACCENT,
      'editor.wordHighlightBackground': lvpaAccent(0.12),
      'editor.wordHighlightStrongBackground': lvpaAccent(0.22),
    },
  },
};

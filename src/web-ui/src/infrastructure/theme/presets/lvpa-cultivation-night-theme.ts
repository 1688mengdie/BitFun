// LVPA 修仙暗色主题 — 太初宗·夜
//
// 基于 china-night-theme 扩展，覆盖 colors 部分追加 LVPA 暗色系统。
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
  overlayBlack,
  rgbFromHex,
  rgbaFromHex,
} from './shared';
import {
  LVPA_COLOR_NIGHT_TOKENS,
} from './lvpa-design-tokens';

// ── LVPA 暗色五色常量 ──
const LVPA_CLOUD_NIGHT = LVPA_COLOR_NIGHT_TOKENS.cloudNight;          // #2a2a2d 墨灰
const LVPA_VERMILLION_NIGHT = LVPA_COLOR_NIGHT_TOKENS.vermillionNight; // #e8494b 殷红
const LVPA_GOLD_NIGHT = LVPA_COLOR_NIGHT_TOKENS.goldNight;            // #b8922e 暗金
const LVPA_JADE_NIGHT = LVPA_COLOR_NIGHT_TOKENS.jadeNight;            // #5a8f7a 黛青

// ── 派生常量 ──
const LVPA_ACCENT = LVPA_JADE_NIGHT;
const LVPA_ACCENT_HOVER = '#4a7a64';
const LVPA_SECONDARY = LVPA_GOLD_NIGHT;
const LVPA_SECONDARY_HOVER = '#9a7a28';
const LVPA_TEXT_PRIMARY = '#e8e8e8';
const LVPA_BUTTON_TEXT = '#c5c3be';
const LVPA_SUCCESS = '#6bc072';
const LVPA_WARNING = LVPA_GOLD_NIGHT;
const LVPA_ERROR = LVPA_VERMILLION_NIGHT;
const LVPA_INFO = LVPA_JADE_NIGHT;

const lvpaNightText = (alpha: number | string) => rgbaFromHex(LVPA_TEXT_PRIMARY, alpha);
const lvpaNightAccent = (alpha: number | string) => rgbaFromHex(LVPA_ACCENT, alpha);

export const lvpaCultivationNightTheme: ThemeConfig = {
  id: 'lvpa-cultivation-night',
  name: '太初宗·夜',
  type: 'dark',
  description: 'LVPA 修仙暗色主题 — 星夜墨色，月华如水，清幽致远',
  author: 'LVPA Team',
  version: '1.0.0',

  // ── Color System ──
  colors: {
    background: {
      primary: LVPA_CLOUD_NIGHT,
      secondary: '#252527',
      tertiary: '#262626',
      elevated: '#262626',
      workbench: LVPA_CLOUD_NIGHT,
      scene: LVPA_CLOUD_NIGHT,
    },

    text: {
      primary: LVPA_TEXT_PRIMARY,
      secondary: '#c5c3be',
      muted: '#928f89',
      disabled: '#555555',
    },

    // 主强调色从 china-night 的蓝（#73a5cc）→ 黛青（#5a8f7a）
    accent: createAccentScale({ base: LVPA_ACCENT, hover: LVPA_ACCENT_HOVER }),

    // 副强调色从 china-night 的绿（#96c6b4）→ 暗金（#b8922e）
    purple: createSecondaryAccentScale({ base: LVPA_SECONDARY, hover: LVPA_SECONDARY_HOVER }),

    semantic: createSemanticColors({
      success: LVPA_SUCCESS,
      warning: LVPA_WARNING,
      error: LVPA_ERROR,
      info: LVPA_INFO,
      bgAlpha: 0.12,
    }),

    border: {
      subtle: lvpaNightText(0.1),
      base: lvpaNightText(0.16),
      medium: lvpaNightText(0.22),
      strong: lvpaNightText(0.28),
      prominent: lvpaNightText(0.38),
    },

    element: {
      subtle: lvpaNightAccent(0.06),
      soft: lvpaNightAccent(0.09),
      base: lvpaNightAccent(0.12),
      medium: lvpaNightAccent(0.16),
      strong: lvpaNightAccent(0.2),
    },

    git: createGitColors({
      branch: rgbFromHex(LVPA_ACCENT),
      branchBg: lvpaNightAccent(0.12),
      changes: rgbFromHex(LVPA_WARNING),
      added: rgbFromHex(LVPA_SUCCESS),
      deleted: rgbFromHex(LVPA_ERROR),
    }),
  },

  // ── Effects ──
  effects: {
    shadow: {
      xs: `0 1px 2px ${overlayBlack(0.5)}`,
      sm: `0 2px 4px ${overlayBlack(0.6)}`,
      base: `0 4px 8px ${overlayBlack(0.65)}`,
      lg: `0 8px 16px ${overlayBlack(0.7)}`,
      xl: `0 12px 24px ${overlayBlack(0.75)}`,
    },

    blur: {
      subtle: 'blur(4px) saturate(1.1)',
      base: 'blur(8px) saturate(1.15)',
    },

    radius: createCompactRadius(),
    spacing: createStandardSpacing(),

    opacity: {
      disabled: 0.45,
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
          background: lvpaNightAccent(0.24),
          color: '#7eb09b',
          border: 'transparent',
          shadow: 'none',
        },
        hover: {
          background: lvpaNightAccent(0.34),
          color: '#96c6b4',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
        active: {
          background: lvpaNightAccent(0.28),
          color: '#96c6b4',
          border: 'transparent',
          shadow: 'none',
          transform: 'none',
        },
      },
      ghost: {
        default: {
          color: '#9a9a9a',
        },
        hover: {
          background: lvpaNightAccent(0.13),
          color: LVPA_BUTTON_TEXT,
          border: 'transparent',
        },
      },
    },
  },

  // ── Monaco Editor ──
  monaco: {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '928f89', fontStyle: 'italic' },
      { token: 'keyword', foreground: LVPA_ERROR },
      { token: 'string', foreground: LVPA_SUCCESS },
      { token: 'number', foreground: LVPA_WARNING },
      { token: 'type', foreground: LVPA_ACCENT },
      { token: 'class', foreground: LVPA_ACCENT },
      { token: 'function', foreground: LVPA_JADE_NIGHT },
      { token: 'variable', foreground: '#c5c3be' },
      { token: 'constant', foreground: '#d4a574' },
      { token: 'operator', foreground: LVPA_ERROR },
      { token: 'tag', foreground: LVPA_ACCENT },
      { token: 'attribute.name', foreground: LVPA_JADE_NIGHT },
      { token: 'attribute.value', foreground: LVPA_SUCCESS },
    ],
    colors: {
      background: LVPA_CLOUD_NIGHT,
      foreground: LVPA_TEXT_PRIMARY,
      lineHighlight: '#252527',
      selection: lvpaNightAccent(0.25),
      cursor: LVPA_ACCENT,
      'editor.selectionBackground': lvpaNightAccent(0.25),
      'editorCursor.foreground': LVPA_ACCENT,
    },
  },
};

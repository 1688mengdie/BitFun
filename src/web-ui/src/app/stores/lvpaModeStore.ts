/**
 * lvpaModeStore — LVPA 双模式切换全局状态
 *
 * 管理 bitfun ↔ taiji 模式切换，持久化到 localStorage。
 * taiji 模式下自动切换到 LVPA 主题（lvpa-cultivation / lvpa-cultivation-night）。
 */

import { create } from 'zustand';
import { themeService } from '@/infrastructure/theme/core/ThemeService';

const STORAGE_KEY = 'lvpa-mode';

export type LvpaMode = 'bitfun' | 'taiji';

interface LvpaModeState {
  mode: LvpaMode;
  initialized: boolean;

  setMode: (mode: LvpaMode) => void;
  toggleMode: () => void;
  reset: () => void;
}

/**
 * LVPA 亮色主题 ID（需由 R-6-102 注册）。
 * 若尚未注册，applyTheme 会静默失败，保证不崩溃。
 */
const LVPA_THEME_LIGHT = 'lvpa-cultivation';
const LVPA_THEME_DARK = 'lvpa-cultivation-night';

/** 解析当前主题类型，返回对应的 LVPA 主题 ID。 */
function resolveLvpaThemeId(): string {
  const current = themeService.getCurrentTheme();
  return current.type === 'dark' ? LVPA_THEME_DARK : LVPA_THEME_LIGHT;
}

function loadPersistedMode(): LvpaMode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'taiji' || stored === 'bitfun') {
      return stored;
    }
  } catch {
    // localStorage not available
  }
  return 'bitfun';
}

function persistMode(mode: LvpaMode): void {
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // localStorage not available — swallow
  }
}

export const useLvpaModeStore = create<LvpaModeState>((set, get) => ({
  mode: loadPersistedMode(),
  initialized: true,

  setMode: (mode: LvpaMode) => {
    const previous = get().mode;
    if (mode === previous) return;

    set({ mode });
    persistMode(mode);

    if (mode === 'taiji') {
      // 进入 taiji 模式 → 切换到 LVPA 主题
      const lvpaThemeId = resolveLvpaThemeId();
      themeService.applyTheme(lvpaThemeId).catch(() => {
        // LVPA 主题尚未注册（R-6-102 未完成时静默失败）
      });
    }
    // 切回 bitfun 模式时不自动恢复主题——用户已选的主题保留不变
  },

  toggleMode: () => {
    const next: LvpaMode = get().mode === 'bitfun' ? 'taiji' : 'bitfun';
    get().setMode(next);
  },

  reset: () => {
    set({ mode: 'bitfun' });
    persistMode('bitfun');
  },
}));

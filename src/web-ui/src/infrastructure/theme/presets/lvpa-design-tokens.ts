// LVPA Design Tokens
// 量价时空（太极多维量化系统）五色/境界色/灵根色 Token 定义
//
// 架构源：量价时空/四总纲/架构总纲.md §4a
// 色值定义：量价时空/Phase-6-类型契约.md §三

// ── 五色 Token 接口 ──

export interface LvpaColorTokens {
  ink: string;           // #1c1c1f 墨色 — 主文字/暗色底色
  vermillion: string;    // #c8102e 朱砂 — 强调色/危险/信号
  gold: string;          // #d4a843 金线 — 高亮/灵石/稀有
  jade: string;          // #7eb09b 青玉 — 成功/自然/柔和
  cloud: string;         // #faf8f0 云白 — 亮色底色/宣纸
}

// ── 境界色 Token 接口 ──

export interface RealmColorTokens {
  qiRefining: string;            // #8a8a8a    炼气
  foundation: string;            // #6d8a5e    筑基
  goldenCore: string;            // #d4a843    金丹
  nascentSoul: string;           // #5e8ab5    元婴
  divineTransformation: string;  // #b55ea8    化神
  voidRefining: string;          // #8a5eb5    炼虚
  ascension: string;             // #5eb5a8    飞升
}

// ── 灵根色 Token 接口 ──

export interface SpiritRootColorTokens {
  metal: string;  // #d4a843
  wood: string;   // #7eb09b
  water: string;  // #5e8ab5
  fire: string;   // #c8102e
  earth: string;  // #a0885e
}

// ── 常量值 ──

export const LVPA_COLOR_TOKENS: LvpaColorTokens = {
  ink: '#1c1c1f',
  vermillion: '#c8102e',
  gold: '#d4a843',
  jade: '#7eb09b',
  cloud: '#faf8f0',
};

export const LVPA_REALM_COLORS: RealmColorTokens = {
  qiRefining: '#8a8a8a',
  foundation: '#6d8a5e',
  goldenCore: '#d4a843',
  nascentSoul: '#5e8ab5',
  divineTransformation: '#b55ea8',
  voidRefining: '#8a5eb5',
  ascension: '#5eb5a8',
};

export const LVPA_SPIRIT_ROOT_COLORS: SpiritRootColorTokens = {
  metal: '#d4a843',
  wood: '#7eb09b',
  water: '#5e8ab5',
  fire: '#c8102e',
  earth: '#a0885e',
};

// ── Night 主题暗色映射 ──

export interface LvpaColorNightTokens {
  cloudNight: string;        // #2a2a2d 墨灰 — 暗色底色
  inkNight: string;          // #111113 深黑 — 暗色主文字背景
  vermillionNight: string;   // #e8494b 殷红 — 暗色强调色
  goldNight: string;         // #b8922e 暗金 — 暗色高亮
  jadeNight: string;         // #5a8f7a 黛青 — 暗色柔和色
}

export const LVPA_COLOR_NIGHT_TOKENS: LvpaColorNightTokens = {
  cloudNight: '#2a2a2d',
  inkNight: '#111113',
  vermillionNight: '#e8494b',
  goldNight: '#b8922e',
  jadeNight: '#5a8f7a',
};

// ── CSS 自定义属性（--lvpa-*）注入函数 ──

/**
 * 在 document.documentElement 上注入所有 --lvpa-* CSS 自定义属性。
 * 由 ThemeService hook 或在主题切换时调用。
 */
export function injectLvpaCssVars(): void {
  const root = document.documentElement;
  const style = root.style;

  // 五色
  style.setProperty('--lvpa-ink', LVPA_COLOR_TOKENS.ink);
  style.setProperty('--lvpa-vermillion', LVPA_COLOR_TOKENS.vermillion);
  style.setProperty('--lvpa-gold', LVPA_COLOR_TOKENS.gold);
  style.setProperty('--lvpa-jade', LVPA_COLOR_TOKENS.jade);
  style.setProperty('--lvpa-cloud', LVPA_COLOR_TOKENS.cloud);

  // 暗色五色
  style.setProperty('--lvpa-cloud-night', LVPA_COLOR_NIGHT_TOKENS.cloudNight);
  style.setProperty('--lvpa-ink-night', LVPA_COLOR_NIGHT_TOKENS.inkNight);
  style.setProperty('--lvpa-vermillion-night', LVPA_COLOR_NIGHT_TOKENS.vermillionNight);
  style.setProperty('--lvpa-gold-night', LVPA_COLOR_NIGHT_TOKENS.goldNight);
  style.setProperty('--lvpa-jade-night', LVPA_COLOR_NIGHT_TOKENS.jadeNight);

  // 境界色
  style.setProperty('--lvpa-realm-qi', LVPA_REALM_COLORS.qiRefining);
  style.setProperty('--lvpa-realm-foundation', LVPA_REALM_COLORS.foundation);
  style.setProperty('--lvpa-realm-golden-core', LVPA_REALM_COLORS.goldenCore);
  style.setProperty('--lvpa-realm-nascent-soul', LVPA_REALM_COLORS.nascentSoul);
  style.setProperty('--lvpa-realm-divine', LVPA_REALM_COLORS.divineTransformation);
  style.setProperty('--lvpa-realm-void', LVPA_REALM_COLORS.voidRefining);
  style.setProperty('--lvpa-realm-ascension', LVPA_REALM_COLORS.ascension);

  // 灵根色
  style.setProperty('--lvpa-root-metal', LVPA_SPIRIT_ROOT_COLORS.metal);
  style.setProperty('--lvpa-root-wood', LVPA_SPIRIT_ROOT_COLORS.wood);
  style.setProperty('--lvpa-root-water', LVPA_SPIRIT_ROOT_COLORS.water);
  style.setProperty('--lvpa-root-fire', LVPA_SPIRIT_ROOT_COLORS.fire);
  style.setProperty('--lvpa-root-earth', LVPA_SPIRIT_ROOT_COLORS.earth);
}

/**
 * 从 document.documentElement 移除所有 --lvpa-* CSS 自定义属性。
 */
export function removeLvpaCssVars(): void {
  const root = document.documentElement;
  const prefix = '--lvpa-';
  // Collect all --lvpa-* vars before removing to avoid live mutation issues
  const lvpaVars: string[] = [];
  for (let i = 0; i < root.style.length; i++) {
    const name = root.style.item(i);
    if (name.startsWith(prefix)) {
      lvpaVars.push(name);
    }
  }
  lvpaVars.forEach(name => root.style.removeProperty(name));
}

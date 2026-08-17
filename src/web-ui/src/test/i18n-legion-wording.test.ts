// R-WF-15: legion -> workflow wording. These assertions lock the user-facing
// i18n strings (zh-CN / zh-TW / en-US) free of the "legion"/"军团" wording so
// the frontend no longer exposes the old concept. Backend structure (LegionPreset)
// is intentionally untouched and verified separately via git diff.
import { describe, expect, it } from 'vitest';
import zhAgents from '@/locales/zh-CN/scenes/agents.json';
import zhBasics from '@/locales/zh-CN/settings/basics.json';
import zhTwAgents from '@/locales/zh-TW/scenes/agents.json';
import zhTwBasics from '@/locales/zh-TW/settings/basics.json';
import enAgents from '@/locales/en-US/scenes/agents.json';
import enBasics from '@/locales/en-US/settings/basics.json';

function collectStrings(value: unknown, out: string[]): void {
  if (typeof value === 'string') {
    out.push(value);
  } else if (Array.isArray(value)) {
    for (const item of value) collectStrings(item, out);
  } else if (value !== null && typeof value === 'object') {
    for (const child of Object.values(value)) collectStrings(child, out);
  }
}

function stringsOf(...sources: unknown[]): string[] {
  const out: string[] = [];
  for (const source of sources) collectStrings(source, out);
  return out;
}

describe('R-WF-15 legion -> workflow wording (i18n zero-residual)', () => {
  it('zh-CN: no "军团" remains in agents + settings locales', () => {
    const found = stringsOf(zhAgents, zhBasics).filter((s) => s.includes('军团'));
    expect(found).toEqual([]);
  });

  it('zh-TW: no "軍團" remains in agents + settings locales', () => {
    const found = stringsOf(zhTwAgents, zhTwBasics).filter((s) => s.includes('軍團'));
    expect(found).toEqual([]);
  });

  it('en-US: no "legion" (case-insensitive) remains in agents + settings locales', () => {
    const found = stringsOf(enAgents, enBasics).filter((s) => /legion/i.test(s));
    expect(found).toEqual([]);
  });
});

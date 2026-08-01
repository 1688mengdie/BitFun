import { describe, expect, it } from 'vitest';

import {
  TEXT_STROKE_GRADIENT_COLORS,
  buildTextStrokeColorCycle,
} from './TextStrokeEffectGradient';
import { APPEARANCE_DOMAIN_TOKENS } from '@/infrastructure/appearance/appearanceDomainTokens';

const MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE = [
  '#eab308',
  '#ef4444',
  '#3b82f6',
  '#06b6d4',
  '#8b5cf6',
] as const;

describe('TextStrokeEffect color cycles', () => {
  it('keeps gradient animation values closed over the original visual color sequence', () => {
    expect(APPEARANCE_DOMAIN_TOKENS.textStroke).toEqual(MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE);
    expect(TEXT_STROKE_GRADIENT_COLORS).toBe(APPEARANCE_DOMAIN_TOKENS.textStroke);

    const expectedCycle = [
      ...MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE.slice(2),
      ...MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE.slice(0, 2),
      MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE[2],
    ].join('; ');
    expect(buildTextStrokeColorCycle(2)).toBe(expectedCycle);
  });
});

import { describe, expect, it } from 'vitest';
import type { AIModelConfig, ReasoningConfig } from '../types';
import {
  canonicalReasoningConfig,
  validateReasoningConfig,
} from './reasoningPresets';

function model(overrides: Partial<AIModelConfig> = {}): AIModelConfig {
  return {
    name: 'Test provider',
    provider: 'anthropic',
    base_url: 'https://example.test',
    model_name: 'test-model',
    enabled: true,
    category: 'general_chat',
    capabilities: ['text_chat'],
    ...overrides,
  };
}

describe('canonical reasoning presets', () => {
  it('converts legacy mode, effort, and budget into one canonical preset', () => {
    const reasoning = canonicalReasoningConfig(model({
      reasoning_mode: 'adaptive',
      reasoning_effort: 'high',
      thinking_budget_tokens: 12_000,
    }));

    expect(reasoning).toMatchObject({
      catalog: { source: 'auto' },
      default_preset: 'adaptive',
      presets: [{
        id: 'adaptive',
        setting: {
          type: 'sequence',
          settings: [
            { type: 'effort', value: 'high', mode: 'adaptive' },
            { type: 'budget_tokens', value: 12_000, mode: 'adaptive' },
          ],
        },
      }],
    });
  });

  it('keeps canonical reasoning authoritative when legacy fields also exist', () => {
    const canonical: ReasoningConfig = {
      catalog: { source: 'disabled' },
      default_preset: 'custom',
      presets: [{ id: 'custom', setting: { type: 'toggle', enabled: false } }],
    };
    expect(canonicalReasoningConfig(model({
      reasoning: canonical,
      reasoning_mode: 'enabled',
      reasoning_effort: 'high',
    }))).toEqual(canonical);
  });

  it('rejects duplicate IDs, invalid explicit catalog bindings, and missing defaults', () => {
    expect(validateReasoningConfig({
      catalog: { source: 'models_dev', provider: '', model: 'gpt-5' },
      presets: [],
    })).toBe('catalog_binding');
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      presets: [
        { id: 'high', setting: { type: 'effort', value: 'high' } },
        { id: 'high', setting: { type: 'effort', value: 'xhigh' } },
      ],
    })).toBe('duplicate_preset_id');
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      default_preset: 'missing',
      presets: [],
    })).toBe('default_preset');
  });

  it('accepts a generated catalog preset as the model default', () => {
    expect(validateReasoningConfig({
      catalog: { source: 'auto' },
      default_preset: 'high',
      presets: [],
    }, ['low', 'high'])).toBeNull();
  });
});

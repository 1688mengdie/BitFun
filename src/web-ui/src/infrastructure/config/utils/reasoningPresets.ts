import type {
  AIModelConfig,
  ReasoningConfig,
  ReasoningMode,
  ReasoningPreset,
  ReasoningPresetSetting,
} from '../types';

const REASONING_MODES = new Set<ReasoningMode>([
  'default',
  'enabled',
  'disabled',
  'adaptive',
]);

function nonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

export function legacyReasoningConfig(config?: Pick<
  AIModelConfig,
  'reasoning' | 'reasoning_mode' | 'reasoning_effort' | 'thinking_budget_tokens' | 'enable_thinking_process'
> | null): ReasoningConfig | undefined {
  if (config?.reasoning) return config.reasoning;

  const mode: ReasoningMode | undefined = config?.reasoning_mode ?? (
    config?.enable_thinking_process === true ? 'enabled' : undefined
  );
  const effort = nonEmpty(config?.reasoning_effort) ? config.reasoning_effort.trim() : undefined;
  const budget = typeof config?.thinking_budget_tokens === 'number'
    ? config.thinking_budget_tokens
    : undefined;

  if (!mode && !effort && !budget) return undefined;

  const effectiveMode = mode ?? 'default';
  const settings: ReasoningPresetSetting[] = [
    effort
      ? { type: 'effort', value: effort, mode: effectiveMode }
      : { type: 'mode', value: effectiveMode },
    ...(budget
      ? [{ type: 'budget_tokens' as const, value: budget, mode: effectiveMode }]
      : []),
  ];
  const id = effectiveMode === 'disabled'
    ? 'off'
    : effectiveMode === 'adaptive'
      ? 'adaptive'
      : effort
        ? 'legacy-effort'
        : effectiveMode === 'enabled'
          ? 'on'
          : 'auto';

  return {
    catalog: { source: 'auto' },
    default_preset: id,
    presets: [{
      id,
      label: id === 'auto' ? 'Auto' : undefined,
      order: 0,
      setting: settings.length === 1
        ? settings[0]
        : { type: 'sequence', settings },
    }],
  };
}

export function canonicalReasoningConfig(config?: Partial<AIModelConfig> | null): ReasoningConfig {
  return normalizeReasoningConfig(legacyReasoningConfig(config));
}

export function normalizeReasoningConfig(config?: ReasoningConfig | null): ReasoningConfig {
  return {
    catalog: config?.catalog ?? { source: 'auto' },
    default_preset: nonEmpty(config?.default_preset) ? config.default_preset.trim() : undefined,
    presets: Array.isArray(config?.presets) ? config.presets.map(clonePreset) : [],
  };
}

export function clonePreset(preset: ReasoningPreset): ReasoningPreset {
  return {
    ...preset,
    setting: preset.setting ? JSON.parse(JSON.stringify(preset.setting)) : undefined,
  };
}

export function cloneReasoningConfig(config: ReasoningConfig): ReasoningConfig {
  return normalizeReasoningConfig(config);
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function validSetting(setting: ReasoningPresetSetting): boolean {
  switch (setting.type) {
    case 'mode':
      return REASONING_MODES.has(setting.value);
    case 'effort':
      return nonEmpty(setting.value) && (!setting.mode || REASONING_MODES.has(setting.mode));
    case 'toggle':
      return typeof setting.enabled === 'boolean';
    case 'budget_tokens':
      return Number.isSafeInteger(setting.value)
        && setting.value > 0
        && (!setting.mode || REASONING_MODES.has(setting.mode));
    case 'request_patch':
      return isJsonObject(setting.body);
    case 'sequence':
      return setting.settings.length > 0 && setting.settings.every(validSetting);
  }
}

export type ReasoningConfigValidationError =
  | 'catalog_binding'
  | 'preset_id'
  | 'duplicate_preset_id'
  | 'preset_setting'
  | 'default_preset';

export function validateReasoningConfig(
  config?: ReasoningConfig | null,
  additionalPresetIds: Iterable<string> = [],
): ReasoningConfigValidationError | null {
  const normalized = normalizeReasoningConfig(config);
  if (
    normalized.catalog?.source === 'models_dev'
    && (!nonEmpty(normalized.catalog.provider) || !nonEmpty(normalized.catalog.model))
  ) {
    return 'catalog_binding';
  }

  const ids = new Set<string>();
  const selectableIds = new Set(
    Array.from(additionalPresetIds, presetId => presetId.trim()).filter(Boolean),
  );
  for (const preset of normalized.presets ?? []) {
    const id = preset.id.trim();
    if (!id) return 'preset_id';
    if (ids.has(id)) return 'duplicate_preset_id';
    ids.add(id);
    if (!preset.disabled && (!preset.setting || !validSetting(preset.setting))) {
      return 'preset_setting';
    }
    if (!preset.disabled && preset.setting) selectableIds.add(id);
  }

  if (normalized.default_preset && !selectableIds.has(normalized.default_preset)) {
    return 'default_preset';
  }

  return null;
}

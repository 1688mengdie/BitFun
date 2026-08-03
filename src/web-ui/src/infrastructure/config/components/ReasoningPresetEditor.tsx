import React, { useMemo, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, IconButton, Input, NumberInput, Select, Switch, Textarea, type SelectOption } from '@/component-library';
import type {
  ReasoningCatalogProjection,
  ReasoningConfig,
  ReasoningMode,
  ReasoningPreset,
  ReasoningPresetSetting,
} from '../types';
import { cloneReasoningConfig } from '../utils/reasoningPresets';
import './ReasoningPresetEditor.scss';

interface ReasoningPresetEditorProps {
  value: ReasoningConfig;
  onChange: (value: ReasoningConfig) => void;
  generatedProjection?: ReasoningCatalogProjection | null;
  disabled?: boolean;
  onValidationChange?: (invalid: boolean) => void;
}

function defaultSetting(type: string): ReasoningPresetSetting {
  switch (type) {
    case 'toggle': return { type: 'toggle', enabled: true };
    case 'budget_tokens': return { type: 'budget_tokens', value: 8192 };
    case 'request_patch': return { type: 'request_patch', body: {} };
    case 'mode': return { type: 'mode', value: 'enabled' };
    case 'sequence': return { type: 'sequence', settings: [{ type: 'effort', value: 'medium' }] };
    case 'effort':
    default:
      return { type: 'effort', value: 'medium' };
  }
}

function settingType(setting?: ReasoningPresetSetting): string {
  return setting?.type ?? 'effort';
}

function settingJson(setting: ReasoningPresetSetting): string {
  return JSON.stringify(
    setting.type === 'request_patch'
      ? setting.body
      : setting.type === 'sequence'
        ? setting.settings
        : setting,
    null,
    2,
  );
}

function parseJsonObject(value: string): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? parsed as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}

export const ReasoningPresetEditor: React.FC<ReasoningPresetEditorProps> = ({
  value,
  onChange,
  generatedProjection,
  disabled = false,
  onValidationChange,
}) => {
  const { t } = useTranslation('settings/ai-model');
  const [jsonDrafts, setJsonDrafts] = useState<Record<string, string>>({});
  const invalidJsonKeysRef = useRef<Set<string>>(new Set());
  const presets = value.presets ?? [];
  const catalog = value.catalog ?? { source: 'auto' as const };
  const modeOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.modeDefault'), value: 'default' },
    { label: t('reasoningPresets.modeEnabled'), value: 'enabled' },
    { label: t('reasoningPresets.modeDisabled'), value: 'disabled' },
    { label: t('reasoningPresets.modeAdaptive'), value: 'adaptive' },
  ], [t]);
  const settingOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.settingEffort'), value: 'effort' },
    { label: t('reasoningPresets.settingToggle'), value: 'toggle' },
    { label: t('reasoningPresets.settingBudget'), value: 'budget_tokens' },
    { label: t('reasoningPresets.settingPatch'), value: 'request_patch' },
    { label: t('reasoningPresets.settingMode'), value: 'mode' },
    { label: t('reasoningPresets.settingSequence'), value: 'sequence' },
  ], [t]);

  const defaultOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.auto'), value: '' },
    ...Array.from(new Map([
      ...(generatedProjection?.presets ?? [])
        .filter(preset => preset.source !== 'model_config')
        .map(preset => [preset.id, {
          label: `${preset.label || preset.id} (${preset.id})`,
          value: preset.id,
        }] as const),
      ...presets
        .filter(preset => !preset.disabled && Boolean(preset.setting))
        .map(preset => [preset.id, {
          label: `${preset.label?.trim() || preset.id} (${preset.id})`,
          value: preset.id,
        }] as const),
    ]).values()),
  ], [generatedProjection?.presets, presets, t]);

  const update = (next: ReasoningConfig) => onChange(cloneReasoningConfig(next));
  const updatePreset = (index: number, changes: Partial<ReasoningPreset>) => {
    const next = [...presets];
    next[index] = { ...next[index], ...changes };
    update({ ...value, presets: next });
  };
  const updateSetting = (index: number, setting: ReasoningPresetSetting) => {
    setJsonDrafts(previous => {
      const next = { ...previous };
      delete next[presets[index]?.id ?? String(index)];
      return next;
    });
    updatePreset(index, { setting });
  };
  const setJsonValidation = (key: string, invalid: boolean) => {
    if (invalid) invalidJsonKeysRef.current.add(key);
    else invalidJsonKeysRef.current.delete(key);
    onValidationChange?.(invalidJsonKeysRef.current.size > 0);
  };

  const addPreset = () => {
    const id = `custom-${Date.now().toString(36)}`;
    updatePreset(presets.length, {
      id,
      label: id,
      order: presets.length * 10,
      setting: defaultSetting('effort'),
    });
  };

  const movePreset = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= presets.length) return;
    const next = [...presets];
    [next[index], next[target]] = [next[target], next[index]];
    update({
      ...value,
      presets: next.map((preset, order) => ({ ...preset, order: order * 10 })),
    });
  };

  return (
    <div className="bitfun-reasoning-preset-editor" data-testid="settings-reasoning-preset-editor">
      <div className="bitfun-reasoning-preset-editor__binding">
        <div className="bitfun-reasoning-preset-editor__field">
          <span>{t('reasoningPresets.catalogSource')}</span>
          <Select
            value={catalog.source}
            disabled={disabled}
            size="small"
            options={[
              { label: t('reasoningPresets.catalogAuto'), value: 'auto' },
              { label: t('reasoningPresets.catalogModelsDev'), value: 'models_dev' },
              { label: t('reasoningPresets.catalogDisabled'), value: 'disabled' },
            ]}
            onChange={(next) => {
              const source = next as 'auto' | 'models_dev' | 'disabled';
              update({
                ...value,
                catalog: source === 'models_dev'
                  ? {
                      source,
                      provider: catalog.source === 'models_dev' ? catalog.provider : '',
                      model: catalog.source === 'models_dev' ? catalog.model : '',
                    }
                  : { source },
              });
            }}
          />
        </div>
        {catalog.source === 'models_dev' && (
          <>
            <Input
              size="small"
              label={t('reasoningPresets.catalogProvider')}
              value={catalog.provider}
              disabled={disabled}
              onChange={(event) => update({
                ...value,
                catalog: { ...catalog, source: 'models_dev', provider: event.target.value, model: catalog.model },
              })}
            />
            <Input
              size="small"
              label={t('reasoningPresets.catalogModel')}
              value={catalog.model}
              disabled={disabled}
              onChange={(event) => update({
                ...value,
                catalog: { ...catalog, source: 'models_dev', provider: catalog.provider, model: event.target.value },
              })}
            />
          </>
        )}
        <div className="bitfun-reasoning-preset-editor__field">
          <span>{t('reasoningPresets.defaultPreset')}</span>
          <Select
            value={value.default_preset ?? ''}
            disabled={disabled}
            size="small"
            options={defaultOptions}
            onChange={(next) => update({ ...value, default_preset: String(next) || undefined })}
          />
        </div>
      </div>

      {generatedProjection?.status === 'known'
        && (generatedProjection.presets?.some(preset => preset.source !== 'model_config') ?? false)
        && (
        <div className="bitfun-reasoning-preset-editor__generated">
          <div className="bitfun-reasoning-preset-editor__section-title">
            {t('reasoningPresets.generatedTitle')}
          </div>
          <div className="bitfun-reasoning-preset-editor__generated-list">
            {generatedProjection.presets?.filter(preset => preset.source !== 'model_config').map(preset => (
              <span key={preset.id} className="bitfun-reasoning-preset-editor__generated-item">
                {preset.label || preset.id}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="bitfun-reasoning-preset-editor__header">
        <div className="bitfun-reasoning-preset-editor__section-title">{t('reasoningPresets.customTitle')}</div>
        <Button variant="secondary" size="small" disabled={disabled} onClick={addPreset}>
          <Plus size={14} aria-hidden="true" />
          {t('reasoningPresets.add')}
        </Button>
      </div>

      {presets.length === 0 ? (
        <div className="bitfun-reasoning-preset-editor__empty">{t('reasoningPresets.empty')}</div>
      ) : (
        <div className="bitfun-reasoning-preset-editor__list">
          {presets.map((preset, index) => {
            const type = settingType(preset.setting);
            const jsonKey = preset.id || String(index);
            const jsonValue = jsonDrafts[jsonKey] ?? (preset.setting ? settingJson(preset.setting) : '{}');
            const jsonIsValid = type === 'request_patch'
              ? parseJsonObject(jsonValue) !== null
              : (() => {
                  try {
                    const parsed: unknown = JSON.parse(jsonValue);
                    return Array.isArray(parsed) && parsed.length > 0;
                  } catch {
                    return false;
                  }
                })();
            return (
              <div key={`${preset.id}-${index}`} className="bitfun-reasoning-preset-editor__row">
                <div className="bitfun-reasoning-preset-editor__row-head">
                  <Input
                    size="small"
                    aria-label={t('reasoningPresets.id')}
                    value={preset.id}
                    disabled={disabled}
                    onChange={(event) => {
                      setJsonValidation(jsonKey, false);
                      const nextId = event.target.value;
                      const default_preset = value.default_preset === preset.id
                        ? nextId
                        : value.default_preset;
                      const next = [...presets];
                      next[index] = { ...next[index], id: nextId };
                      update({ ...value, default_preset, presets: next });
                    }}
                  />
                  <Input
                    size="small"
                    aria-label={t('reasoningPresets.label')}
                    value={preset.label ?? ''}
                    disabled={disabled}
                    placeholder={t('reasoningPresets.labelPlaceholder')}
                    onChange={(event) => updatePreset(index, { label: event.target.value || undefined })}
                  />
                  <label className="bitfun-reasoning-preset-editor__default-toggle">
                    <input
                      type="radio"
                      name="reasoning-default-preset"
                      checked={value.default_preset === preset.id}
                      disabled={disabled || preset.disabled || !preset.setting}
                      onChange={() => update({ ...value, default_preset: preset.id })}
                    />
                    {t('reasoningPresets.default')}
                  </label>
                  <Switch
                    size="small"
                    checked={!preset.disabled}
                    disabled={disabled}
                    aria-label={t('reasoningPresets.enabled')}
                    onChange={(event) => updatePreset(index, {
                      disabled: !event.target.checked,
                      setting: event.target.checked ? (preset.setting ?? defaultSetting('effort')) : preset.setting,
                    })}
                  />
                  <IconButton
                    size="small"
                    variant="ghost"
                    tooltip={t('reasoningPresets.moveUp')}
                    disabled={disabled || index === 0}
                    onClick={() => movePreset(index, -1)}
                  >
                    <ArrowUp size={14} />
                  </IconButton>
                  <IconButton
                    size="small"
                    variant="ghost"
                    tooltip={t('reasoningPresets.moveDown')}
                    disabled={disabled || index === presets.length - 1}
                    onClick={() => movePreset(index, 1)}
                  >
                    <ArrowDown size={14} />
                  </IconButton>
                  <IconButton
                    size="small"
                    variant="ghost"
                    tooltip={t('reasoningPresets.remove')}
                    disabled={disabled}
                    onClick={() => {
                      setJsonValidation(jsonKey, false);
                      const next = presets.filter((_, itemIndex) => itemIndex !== index);
                      update({
                        ...value,
                        presets: next,
                        default_preset: value.default_preset === preset.id ? undefined : value.default_preset,
                      });
                    }}
                  >
                    <Trash2 size={14} />
                  </IconButton>
                </div>

                {!preset.disabled && (
                  <div className="bitfun-reasoning-preset-editor__setting">
                    <Select
                      size="small"
                      value={type}
                      disabled={disabled}
                      options={settingOptions}
                      onChange={(next) => {
                        setJsonValidation(jsonKey, false);
                        updateSetting(index, defaultSetting(String(next)));
                      }}
                    />
                    {type === 'effort' && preset.setting?.type === 'effort' && (
                      <>
                        <Input size="small" value={preset.setting.value} disabled={disabled} onChange={(event) => updateSetting(index, { ...preset.setting!, type: 'effort', value: event.target.value })} />
                        <Select size="small" value={preset.setting.mode ?? ''} disabled={disabled} placeholder={t('reasoningPresets.modeOptional')} options={[{ label: t('reasoningPresets.modeOptional'), value: '' }, ...modeOptions]} onChange={(next) => updateSetting(index, { type: 'effort', value: preset.setting!.type === 'effort' ? preset.setting!.value : '', mode: String(next) ? String(next) as ReasoningMode : undefined })} />
                      </>
                    )}
                    {type === 'toggle' && preset.setting?.type === 'toggle' && (
                      <Switch size="small" checked={preset.setting.enabled} disabled={disabled} onChange={(event) => updateSetting(index, { type: 'toggle', enabled: event.target.checked })} />
                    )}
                    {type === 'budget_tokens' && preset.setting?.type === 'budget_tokens' && (
                      <>
                        <NumberInput size="small" value={preset.setting.value} min={1} max={2_000_000_000} step={1024} disabled={disabled} disableWheel onChange={(next) => updateSetting(index, { ...preset.setting!, type: 'budget_tokens', value: next })} />
                        <Select size="small" value={preset.setting.mode ?? ''} disabled={disabled} placeholder={t('reasoningPresets.modeOptional')} options={[{ label: t('reasoningPresets.modeOptional'), value: '' }, ...modeOptions]} onChange={(next) => updateSetting(index, { type: 'budget_tokens', value: preset.setting!.type === 'budget_tokens' ? preset.setting!.value : 8192, mode: String(next) ? String(next) as ReasoningMode : undefined })} />
                      </>
                    )}
                    {type === 'mode' && preset.setting?.type === 'mode' && (
                      <Select size="small" value={preset.setting.value} disabled={disabled} options={modeOptions} onChange={(next) => updateSetting(index, { type: 'mode', value: next as ReasoningMode })} />
                    )}
                    {(type === 'request_patch' || type === 'sequence') && (
                      <div className="bitfun-reasoning-preset-editor__json">
                        <Textarea
                          value={jsonValue}
                          disabled={disabled}
                          rows={4}
                          error={!jsonIsValid}
                          errorMessage={!jsonIsValid ? t('reasoningPresets.invalidJson') : undefined}
                          onChange={(event) => {
                            const nextText = event.target.value;
                            setJsonDrafts(previous => ({ ...previous, [jsonKey]: nextText }));
                            try {
                              const parsed: unknown = JSON.parse(nextText);
                              if (type === 'request_patch') {
                                if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
                                  setJsonValidation(jsonKey, false);
                                  updatePreset(index, { setting: { type: 'request_patch', body: parsed as Record<string, unknown> } });
                                } else {
                                  setJsonValidation(jsonKey, true);
                                }
                              } else if (Array.isArray(parsed) && parsed.length > 0) {
                                setJsonValidation(jsonKey, false);
                                updatePreset(index, { setting: { type: 'sequence', settings: parsed as ReasoningPresetSetting[] } });
                              } else {
                                setJsonValidation(jsonKey, true);
                              }
                            } catch {
                              setJsonValidation(jsonKey, true);
                            }
                          }}
                        />
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default ReasoningPresetEditor;

import React, { useMemo, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  IconButton,
  Input,
  NumberInput,
  Select,
  Switch,
  Textarea,
  type SelectOption,
} from '@/component-library';
import type {
  ReasoningCatalogProjection,
  ReasoningConfig,
  ReasoningPreset,
  ReasoningPresetAction,
} from '../types';
import {
  availableReasoningActionTypes,
  cloneReasoningConfig,
  nextReasoningActionType,
} from '../utils/reasoningPresets';
import './ReasoningPresetEditor.scss';

interface ReasoningPresetEditorProps {
  value: ReasoningConfig;
  onChange: (value: ReasoningConfig) => void;
  generatedProjection?: ReasoningCatalogProjection | null;
  disabled?: boolean;
  onValidationChange?: (invalid: boolean) => void;
}

function defaultAction(type: ReasoningPresetAction['type']): ReasoningPresetAction {
  switch (type) {
    case 'toggle': return { type, enabled: true };
    case 'budget_tokens': return { type, value: 8192 };
    case 'request_patch': return { type, body: {} };
    case 'effort': return { type, value: 'medium' };
  }
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
  const actionOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.settingEffort'), value: 'effort' },
    { label: t('reasoningPresets.settingToggle'), value: 'toggle' },
    { label: t('reasoningPresets.settingBudget'), value: 'budget_tokens' },
    { label: t('reasoningPresets.settingPatch'), value: 'request_patch' },
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
        .filter(preset => !preset.disabled && Boolean(preset.actions?.length))
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
  const updateAction = (presetIndex: number, actionIndex: number, action: ReasoningPresetAction) => {
    const actions = [...(presets[presetIndex]?.actions ?? [])];
    actions[actionIndex] = action;
    updatePreset(presetIndex, { actions });
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
      actions: [defaultAction('effort')],
    });
  };

  const movePreset = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= presets.length) return;
    const next = [...presets];
    [next[index], next[target]] = [next[target], next[index]];
    update({ ...value, presets: next.map((preset, order) => ({ ...preset, order: order * 10 })) });
  };

  const moveAction = (presetIndex: number, actionIndex: number, direction: -1 | 1) => {
    const actions = [...(presets[presetIndex]?.actions ?? [])];
    const target = actionIndex + direction;
    if (target < 0 || target >= actions.length) return;
    [actions[actionIndex], actions[target]] = [actions[target], actions[actionIndex]];
    updatePreset(presetIndex, { actions });
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
                catalog: { ...catalog, provider: event.target.value },
              })}
            />
            <Input
              size="small"
              label={t('reasoningPresets.catalogModel')}
              value={catalog.model}
              disabled={disabled}
              onChange={(event) => update({
                ...value,
                catalog: { ...catalog, model: event.target.value },
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
          {presets.map((preset, presetIndex) => (
            <div key={`${preset.id}-${presetIndex}`} className="bitfun-reasoning-preset-editor__row">
              <div className="bitfun-reasoning-preset-editor__row-head">
                <Input
                  size="small"
                  aria-label={t('reasoningPresets.id')}
                  value={preset.id}
                  disabled={disabled}
                  onChange={(event) => {
                    const nextId = event.target.value;
                    updatePreset(presetIndex, { id: nextId });
                    if (value.default_preset === preset.id) {
                      update({
                        ...value,
                        default_preset: nextId,
                        presets: presets.map((item, index) => index === presetIndex ? { ...item, id: nextId } : item),
                      });
                    }
                  }}
                />
                <Input
                  size="small"
                  aria-label={t('reasoningPresets.label')}
                  value={preset.label ?? ''}
                  disabled={disabled}
                  placeholder={t('reasoningPresets.labelPlaceholder')}
                  onChange={(event) => updatePreset(presetIndex, { label: event.target.value || undefined })}
                />
                <label className="bitfun-reasoning-preset-editor__default-toggle">
                  <input
                    type="radio"
                    name="reasoning-default-preset"
                    checked={value.default_preset === preset.id}
                    disabled={disabled || preset.disabled || !preset.actions?.length}
                    onChange={() => update({ ...value, default_preset: preset.id })}
                  />
                  {t('reasoningPresets.default')}
                </label>
                <Switch
                  size="small"
                  checked={!preset.disabled}
                  disabled={disabled}
                  aria-label={t('reasoningPresets.enabled')}
                  onChange={(event) => updatePreset(presetIndex, {
                    disabled: !event.target.checked,
                    actions: event.target.checked && !preset.actions?.length
                      ? [defaultAction('effort')]
                      : preset.actions,
                  })}
                />
                <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveUp')} disabled={disabled || presetIndex === 0} onClick={() => movePreset(presetIndex, -1)}><ArrowUp size={14} /></IconButton>
                <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveDown')} disabled={disabled || presetIndex === presets.length - 1} onClick={() => movePreset(presetIndex, 1)}><ArrowDown size={14} /></IconButton>
                <IconButton
                  size="small"
                  variant="ghost"
                  tooltip={t('reasoningPresets.remove')}
                  disabled={disabled}
                  onClick={() => update({
                    ...value,
                    presets: presets.filter((_, index) => index !== presetIndex),
                    default_preset: value.default_preset === preset.id ? undefined : value.default_preset,
                  })}
                >
                  <Trash2 size={14} />
                </IconButton>
              </div>

              {!preset.disabled && (
                <div className="bitfun-reasoning-preset-editor__actions">
                  {(preset.actions ?? []).map((action, actionIndex) => {
                    const jsonKey = `${preset.id || presetIndex}:${actionIndex}`;
                    const jsonValue = jsonDrafts[jsonKey]
                      ?? (action.type === 'request_patch' ? JSON.stringify(action.body, null, 2) : '{}');
                    const jsonIsValid = action.type !== 'request_patch' || parseJsonObject(jsonValue) !== null;
                    return (
                      <div key={jsonKey} className="bitfun-reasoning-preset-editor__action">
                        <Select
                          size="small"
                          value={action.type}
                          disabled={disabled}
                          options={actionOptions.filter(option => (
                            availableReasoningActionTypes(preset.actions ?? [], actionIndex)
                              .includes(option.value as ReasoningPresetAction['type'])
                          ))}
                          onChange={(next) => {
                            setJsonValidation(jsonKey, false);
                            updateAction(presetIndex, actionIndex, defaultAction(next as ReasoningPresetAction['type']));
                          }}
                        />
                        {action.type === 'effort' && (
                          <Input size="small" value={action.value} disabled={disabled} onChange={(event) => updateAction(presetIndex, actionIndex, { type: 'effort', value: event.target.value })} />
                        )}
                        {action.type === 'toggle' && (
                          <Switch size="small" checked={action.enabled} disabled={disabled} onChange={(event) => updateAction(presetIndex, actionIndex, { type: 'toggle', enabled: event.target.checked })} />
                        )}
                        {action.type === 'budget_tokens' && (
                          <NumberInput size="small" value={action.value} min={1} max={2_000_000_000} step={1024} disabled={disabled} disableWheel onChange={(next) => updateAction(presetIndex, actionIndex, { type: 'budget_tokens', value: next })} />
                        )}
                        {action.type === 'request_patch' && (
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
                                const body = parseJsonObject(nextText);
                                setJsonValidation(jsonKey, !body);
                                if (body) updateAction(presetIndex, actionIndex, { type: 'request_patch', body });
                              }}
                            />
                          </div>
                        )}
                        <div className="bitfun-reasoning-preset-editor__action-controls">
                          <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveUp')} disabled={disabled || actionIndex === 0} onClick={() => moveAction(presetIndex, actionIndex, -1)}><ArrowUp size={14} /></IconButton>
                          <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveDown')} disabled={disabled || actionIndex === (preset.actions?.length ?? 0) - 1} onClick={() => moveAction(presetIndex, actionIndex, 1)}><ArrowDown size={14} /></IconButton>
                          <IconButton
                            size="small"
                            variant="ghost"
                            tooltip={t('reasoningPresets.remove')}
                            disabled={disabled || (preset.actions?.length ?? 0) <= 1}
                            onClick={() => updatePreset(presetIndex, { actions: preset.actions?.filter((_, index) => index !== actionIndex) })}
                          >
                            <Trash2 size={14} />
                          </IconButton>
                        </div>
                      </div>
                    );
                  })}
                  <Button
                    variant="secondary"
                    size="small"
                    disabled={disabled}
                    onClick={() => updatePreset(presetIndex, {
                      actions: [
                        ...(preset.actions ?? []),
                        defaultAction(nextReasoningActionType(preset.actions ?? [])),
                      ],
                    })}
                  >
                    <Plus size={14} aria-hidden="true" />
                    {t('reasoningPresets.addAction')}
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default ReasoningPresetEditor;

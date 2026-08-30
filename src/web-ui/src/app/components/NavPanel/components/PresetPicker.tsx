/**
 * PresetPicker - group chat workflow preset selector (GROUP P2).
 *
 * Merge source (owner contract, client-side merge, no backend authoring):
 * - Built-in PATTERNS (orchestration-patterns.ts, 18 templates).
 * - Saved legion presets (LegionPresetAPI.listPresets -> <user-config>/legions/<id>.json).
 *
 * Selection semantics:
 * - value === null => "no preset" (create dialog falls back to manual member mode).
 * - Selecting a preset surfaces it via onChange(pattern, isSaved): isSaved=true means the
 *   preset id already resolves on disk (a saved preset), so the create flow can pass its id
 *   as `preset_id` directly. isSaved=false means a built-in template NOT yet persisted, so
 *   the create flow must materialize it (LegionPresetAPI.createPreset) before create.
 *
 * Reuse:
 * - LegionPresetAPI (existing service-api), PATTERNS (existing data module).
 * - Member preview renders preset.nodes (role + agent + gate), the same member model the
 *   backend create_group_from_preset consumes per node.agent.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import {
  LegionPresetAPI,
  type CreatePresetRequest,
} from '@/infrastructure/api/service-api/LegionPresetAPI';
import PATTERNS, { type LegionPattern } from '@/app/scenes/agents/data/orchestration-patterns';
import './PresetPicker.scss';

interface PresetPickerProps {
  /** Currently selected workflow preset; null = manual member mode. */
  value: LegionPattern | null;
  /**
   * Selection callback. `isSaved` = the preset id already resolves on disk (a
   * saved preset), so the create flow does not need to materialize it.
   */
  onChange: (pattern: LegionPattern | null, isSaved: boolean) => void;
  disabled?: boolean;
}

/** Convert a saved legion preset (backend shape) into the built-in pattern shape.
 *  Same adapter semantics as AgentsScene.presetToPattern; complexityLevel is
 *  absent from persisted presets and defaults to the node-count floor. */
function presetToPattern(preset: CreatePresetRequest): LegionPattern {
  const complexityLevel = Math.min(7, Math.max(1, Math.ceil((preset.nodes?.length ?? 0) / 2)));
  return {
    id: preset.id,
    name: preset.name,
    description: preset.description,
    complexityLevel,
    nodes: (preset.nodes ?? []).map((n) => ({
      id: n.id,
      agent: n.agent,
      role: n.role,
      prompt: n.prompt,
      gate: n.gate,
    })),
    edges: (preset.edges ?? []).map((e) => ({
      from: e.from,
      to: e.to,
      condition: e.condition,
    })),
  };
}

/** Merge built-in patterns with saved presets, preferring saved presets on id
 *  collision (a user-persisted template overrides the built-in default). */
function mergePatterns(saved: CreatePresetRequest[]): {
  patterns: LegionPattern[];
  savedIds: Set<string>;
} {
  const byId = new Map<string, LegionPattern>();
  for (const pattern of PATTERNS) {
    byId.set(pattern.id, pattern);
  }
  const savedIds = new Set<string>();
  for (const preset of saved ?? []) {
    if (!preset || !preset.id) continue;
    byId.set(preset.id, presetToPattern(preset));
    savedIds.add(preset.id);
  }
  return { patterns: Array.from(byId.values()), savedIds };
}

export const PresetPicker: React.FC<PresetPickerProps> = ({
  value,
  onChange,
  disabled = false,
}) => {
  const { t } = useI18n(['common', 'scenes/agents']);
  const [patterns, setPatterns] = useState<LegionPattern[]>(PATTERNS);
  const [savedIds, setSavedIds] = useState<Set<string>>(new Set());
  const [loadFailed, setLoadFailed] = useState(false);

  // Mount-only load: presets are static data; the effect must not depend on unstable
  // identities (notification/t) or it would retrigger on every render.
  useEffect(() => {
    let cancelled = false;
    setLoadFailed(false);
    LegionPresetAPI.listPresets()
      .then((saved) => {
        if (cancelled) return;
        const merged = mergePatterns(saved ?? []);
        setPatterns(merged.patterns);
        setSavedIds(merged.savedIds);
      })
      .catch(() => {
        if (cancelled) return;
        // Fail-safe: fall back to built-in templates only, still usable.
        setLoadFailed(true);
        setPatterns(PATTERNS);
        setSavedIds(new Set());
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleSelect = useCallback(
    (pattern: LegionPattern | null) => {
      if (disabled) return;
      onChange(pattern, pattern ? savedIds.has(pattern.id) : false);
    },
    [disabled, onChange, savedIds],
  );

  const patternName = useCallback(
    (pattern: LegionPattern) =>
      t(`scenes/agents:legionPattern.patterns.${pattern.id}.name`, { defaultValue: pattern.name }),
    [t],
  );

  const patternDescription = useCallback(
    (pattern: LegionPattern) =>
      t(`scenes/agents:legionPattern.patterns.${pattern.id}.description`, {
        defaultValue: pattern.description,
      }),
    [t],
  );

  const selectedEmpty = value === null;

  const memberPreview = useMemo(() => {
    if (!value) return [];
    return value.nodes ?? [];
  }, [value]);

  return (
    <div className="preset-picker">
      <div className="preset-picker__header">
        <span className="preset-picker__label">{t('nav.groupChats.presetWorkflow')}</span>
      </div>

      {loadFailed ? (
        <div className="preset-picker__state" data-testid="preset-load-failed">
          {t('nav.groupChats.presetLoadFailed')}
        </div>
      ) : null}

      <div className="preset-picker__list" role="radiogroup" aria-label={t('nav.groupChats.presetWorkflow')}>
        <button
          type="button"
          role="radio"
          aria-checked={selectedEmpty}
          data-testid="preset-option-none"
          className={`preset-picker__option${selectedEmpty ? ' is-selected' : ''}`}
          onClick={() => handleSelect(null)}
          disabled={disabled}
        >
          <span className="preset-picker__option-title">{t('nav.groupChats.noPresetSelection')}</span>
          <span className="preset-picker__option-sub">{t('nav.groupChats.presetWorkflowHint')}</span>
        </button>

        {patterns.map((pattern) => {
          const isSelected = value?.id === pattern.id;
          return (
            <button
              key={pattern.id}
              type="button"
              role="radio"
              aria-checked={isSelected}
              data-testid="preset-option"
              data-preset-id={pattern.id}
              className={`preset-picker__option${isSelected ? ' is-selected' : ''}`}
              onClick={() => handleSelect(pattern)}
              disabled={disabled}
            >
              <span className="preset-picker__option-title">{patternName(pattern)}</span>
              <span className="preset-picker__option-sub">
                {patternDescription(pattern)}
                <span className="preset-picker__option-count">
                  {t('nav.groupChats.presetMembersCount', { count: pattern.nodes.length })}
                </span>
              </span>
            </button>
          );
        })}
      </div>

      {value && memberPreview.length > 0 ? (
        <div className="preset-picker__preview">
          <div className="preset-picker__preview-label">
            {t('nav.groupChats.presetMembersCount', { count: memberPreview.length })}
          </div>
          <div className="preset-picker__member-list">
            {memberPreview.map((node, index) => (
              <div
                key={node.id ?? index}
                className="preset-picker__member-row"
                data-testid="preset-member-preview"
              >
                <span className="preset-picker__member-role">{node.role || node.id}</span>
                <span className="preset-picker__member-agent">{node.agent}</span>
                {node.gate ? (
                  <span className="preset-picker__member-gate">{t('nav.groupChats.presetGate')}</span>
                ) : null}
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
};

export default PresetPicker;

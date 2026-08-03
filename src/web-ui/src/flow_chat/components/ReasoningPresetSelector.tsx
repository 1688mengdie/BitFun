import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Brain, Check, ChevronDown } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import type {
  ReasoningCatalogProjection,
  ReasoningPresetDescriptor,
} from '@/infrastructure/config/types';
import { getModelSelectorDropdownStyle } from './modelSelectorDropdownPosition';
import './ReasoningPresetSelector.scss';

interface ReasoningPresetSelectorProps {
  projection?: ReasoningCatalogProjection | null;
  selectedPreset?: string | null;
  disabled?: boolean;
  loading?: boolean;
  dropdownPlacement?: 'top' | 'bottom';
  onSelect: (presetId: string | null) => void | Promise<void>;
}

function presetLabel(
  preset: ReasoningPresetDescriptor,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  return t(`reasoningEffort.${preset.id}`, { defaultValue: preset.label || preset.id });
}

export const ReasoningPresetSelector: React.FC<ReasoningPresetSelectorProps> = ({
  projection,
  selectedPreset,
  disabled = false,
  loading = false,
  dropdownPlacement = 'top',
  onSelect,
}) => {
  const { t } = useTranslation('flow-chat');
  const [open, setOpen] = useState(false);
  const [keyboardOpen, setKeyboardOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuId = useId();
  const [menuStyle, setMenuStyle] = useState<React.CSSProperties>({
    position: 'fixed',
    visibility: 'hidden',
  });

  const presets = useMemo(
    () => (projection?.status === 'known' ? projection.presets ?? [] : []),
    [projection],
  );
  const selected = presets.find(preset => preset.id === selectedPreset);
  const defaultPreset = presets.find(preset => preset.id === projection?.default_preset);

  useEffect(() => {
    if (presets.length === 0) setOpen(false);
  }, [presets.length]);

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !menuRef.current?.contains(target)) {
        setOpen(false);
        setKeyboardOpen(false);
      }
    };
    document.addEventListener('mousedown', handlePointerDown);
    return () => document.removeEventListener('mousedown', handlePointerDown);
  }, [open]);

  useEffect(() => {
    if (!open || !rootRef.current) return;
    const updatePosition = () => {
      if (!rootRef.current || !menuRef.current) return;
      setMenuStyle(getModelSelectorDropdownStyle(
        rootRef.current.getBoundingClientRect(),
        menuRef.current.getBoundingClientRect(),
        dropdownPlacement,
        { width: window.innerWidth, height: window.innerHeight },
      ));
    };
    updatePosition();
    const observer = new ResizeObserver(updatePosition);
    if (menuRef.current) observer.observe(menuRef.current);
    window.addEventListener('scroll', updatePosition, true);
    window.addEventListener('resize', updatePosition);
    return () => {
      observer.disconnect();
      window.removeEventListener('scroll', updatePosition, true);
      window.removeEventListener('resize', updatePosition);
    };
  }, [dropdownPlacement, open]);

  useEffect(() => {
    if (!open || !keyboardOpen) return;
    const frame = window.requestAnimationFrame(() => {
      const checked = menuRef.current?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"][aria-checked="true"]',
      );
      const first = menuRef.current?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"]',
      );
      (checked ?? first)?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [keyboardOpen, open]);

  const select = useCallback((presetId: string | null) => {
    setOpen(false);
    setKeyboardOpen(false);
    void onSelect(presetId);
  }, [onSelect]);

  const handleMenuKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      setOpen(false);
      setKeyboardOpen(false);
      triggerRef.current?.focus();
      return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>(
      'button[role="menuitemradio"]:not(:disabled)',
    ));
    if (items.length === 0) return;
    event.preventDefault();
    const activeIndex = items.indexOf(document.activeElement as HTMLButtonElement);
    let nextIndex = activeIndex;
    if (event.key === 'Home') nextIndex = 0;
    if (event.key === 'End') nextIndex = items.length - 1;
    if (event.key === 'ArrowDown') nextIndex = activeIndex < 0 ? 0 : (activeIndex + 1) % items.length;
    if (event.key === 'ArrowUp') nextIndex = activeIndex < 0 ? items.length - 1 : (activeIndex - 1 + items.length) % items.length;
    items[nextIndex]?.focus();
  }, []);

  if (presets.length === 0) return null;

  const currentLabel = selected
    ? presetLabel(selected, t)
    : t('reasoningSelector.auto');
  const tooltip = selected
    ? t('reasoningSelector.current', { preset: currentLabel })
    : t('reasoningSelector.currentAuto', {
        preset: defaultPreset ? presetLabel(defaultPreset, t) : t('reasoningSelector.modelDefault'),
      });

  return (
    <div ref={rootRef} className="bitfun-reasoning-preset-selector">
      <Tooltip content={tooltip}>
        <button
          ref={triggerRef}
          type="button"
          className={`bitfun-reasoning-preset-selector__trigger${open ? ' bitfun-reasoning-preset-selector__trigger--open' : ''}`}
          data-testid="chat-reasoning-preset-selector-btn"
          aria-haspopup="menu"
          aria-expanded={open}
          aria-controls={open ? menuId : undefined}
          disabled={disabled || loading}
          onClick={(event) => {
            const nextOpen = !open;
            setKeyboardOpen(nextOpen && event.detail === 0);
            setOpen(nextOpen);
          }}
          onKeyDown={(event) => {
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault();
              setKeyboardOpen(true);
              setOpen(true);
            } else if (event.key === 'Escape' && open) {
              event.preventDefault();
              setOpen(false);
              setKeyboardOpen(false);
            }
          }}
        >
          <Brain size={10} aria-hidden="true" />
          <span className="bitfun-reasoning-preset-selector__label">{currentLabel}</span>
          <ChevronDown size={10} aria-hidden="true" />
        </button>
      </Tooltip>

      {open && createPortal(
        <div
          id={menuId}
          ref={menuRef}
          className="bitfun-reasoning-preset-selector__menu"
          style={menuStyle}
          role="menu"
          aria-label={t('reasoningSelector.title')}
          data-testid="chat-reasoning-preset-selector-menu"
          onKeyDown={handleMenuKeyDown}
        >
          <div className="bitfun-reasoning-preset-selector__header">
            {t('reasoningSelector.title')}
          </div>
          <button
            type="button"
            role="menuitemradio"
            aria-checked={!selected}
            className="bitfun-reasoning-preset-selector__option"
            onClick={() => select(null)}
          >
            <span>
              <strong>{t('reasoningSelector.auto')}</strong>
              {defaultPreset && (
                <small>{presetLabel(defaultPreset, t)}</small>
              )}
            </span>
            {!selected && <Check size={14} aria-hidden="true" />}
          </button>
          {presets.map(preset => {
            const isSelected = selected?.id === preset.id;
            return (
              <button
                key={preset.id}
                type="button"
                role="menuitemradio"
                aria-checked={isSelected}
                data-preset-id={preset.id}
                className="bitfun-reasoning-preset-selector__option"
                onClick={() => select(preset.id)}
              >
                <span>
                  <strong>{presetLabel(preset, t)}</strong>
                  <small>{t(`reasoningSelector.source.${preset.source}`)}</small>
                </span>
                {isSelected && <Check size={14} aria-hidden="true" />}
              </button>
            );
          })}
        </div>,
        getAppearanceOverlayHost(),
      )}
    </div>
  );
};

export default ReasoningPresetSelector;

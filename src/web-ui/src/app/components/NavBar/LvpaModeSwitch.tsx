/**
 * LvpaModeSwitch — bitfun ↔ taiji 双模式切换入口
 *
 * 位于 NavBar 中，点击切换位面模式。
 * bitfun 模式显示 "BitFun" 缩写，taiji 模式显示 "太初宗" 缩写。
 */

import React, { useCallback, useRef, useState, useEffect } from 'react';
import { SwitchCamera } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useLvpaModeStore } from '../../stores/lvpaModeStore';
import './LvpaModeSwitch.scss';

export const LvpaModeSwitch: React.FC = () => {
  const mode = useLvpaModeStore(s => s.mode);
  const toggleMode = useLvpaModeStore(s => s.toggleMode);
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  const isTaiji = mode === 'taiji';
  const label = isTaiji ? '太初宗' : 'BitFun';

  // 点击外部关闭菜单
  useEffect(() => {
    if (!open) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (
        menuRef.current &&
        !menuRef.current.contains(e.target as Node) &&
        triggerRef.current &&
        !triggerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [open]);

  const handleToggle = useCallback(() => {
    toggleMode();
    setOpen(false);
  }, [toggleMode]);

  return (
    <div className="lvpa-mode-switch" ref={menuRef}>
      <Tooltip content={isTaiji ? '切换到 BitFun 模式' : '切换到太极模式'} placement="bottom">
        <button
          ref={triggerRef}
          type="button"
          className={`lvpa-mode-switch__trigger${isTaiji ? ' lvpa-mode-switch__trigger--taiji' : ''}`}
          onClick={() => setOpen(prev => !prev)}
          aria-label={`当前模式: ${label}，点击切换`}
          aria-expanded={open}
          aria-haspopup="menu"
        >
          <SwitchCamera size={14} className="lvpa-mode-switch__icon" />
          <span className="lvpa-mode-switch__label">{label}</span>
        </button>
      </Tooltip>

      {open && (
        <div className="lvpa-mode-switch__menu" role="menu">
          <button
            type="button"
            className={`lvpa-mode-switch__item${!isTaiji ? ' lvpa-mode-switch__item--active' : ''}`}
            onClick={() => { if (isTaiji) handleToggle(); setOpen(false); }}
            role="menuitem"
            disabled={!isTaiji}
          >
            <span className="lvpa-mode-switch__item-icon">B</span>
            <span className="lvpa-mode-switch__item-label">BitFun 模式</span>
            {!isTaiji && <span className="lvpa-mode-switch__item-check">✓</span>}
          </button>
          <button
            type="button"
            className={`lvpa-mode-switch__item${isTaiji ? ' lvpa-mode-switch__item--active' : ''}`}
            onClick={() => { if (!isTaiji) handleToggle(); setOpen(false); }}
            role="menuitem"
            disabled={isTaiji}
          >
            <span className="lvpa-mode-switch__item-icon">太</span>
            <span className="lvpa-mode-switch__item-label">太极模式</span>
            {isTaiji && <span className="lvpa-mode-switch__item-check">✓</span>}
          </button>
        </div>
      )}
    </div>
  );
};

export default LvpaModeSwitch;

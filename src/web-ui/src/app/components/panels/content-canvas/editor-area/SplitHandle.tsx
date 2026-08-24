/**
 * SplitHandle component.
 * Divider for adjusting split ratio.
 */

import React, { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { LAYOUT_CONFIG } from '../types';
import './SplitHandle.scss';

export interface SplitHandleProps {
  /** Split direction */
  direction: 'horizontal' | 'vertical';
  /** Current ratio */
  ratio: number;
  /** Ratio change callback */
  onRatioChange: (ratio: number) => void;
  /** Container ref */
  containerRef: React.RefObject<HTMLElement>;
  /** Extra inline styles (e.g. explicit CSS Grid placement) */
  style?: React.CSSProperties;
  /** Upper bound for the ratio while dragging (defaults to LAYOUT_CONFIG.MAX_SPLIT_RATIO). */
  minRatio?: number;
  /** Lower bound for the ratio while dragging (defaults to LAYOUT_CONFIG.MIN_SPLIT_RATIO). */
  maxRatio?: number;
  /** Ratio to restore on double-click (defaults to LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO). */
  resetRatio?: number;
}

export const SplitHandle: React.FC<SplitHandleProps> = ({
  direction,
  ratio,
  onRatioChange,
  containerRef,
  style,
  minRatio,
  maxRatio,
  resetRatio,
}) => {
  const { t } = useTranslation('components');
  const [isDragging, setIsDragging] = useState(false);
  const startPosRef = useRef(0);
  const startRatioRef = useRef(ratio);

  // Effective bounds for this handle. grid9 resizers pass explicit
  // minRatio/maxRatio (from GRID9_RATIO_CONFIG); legacy splits fall back to the
  // layout config so they keep today's behaviour.
  const effectiveMin = minRatio ?? LAYOUT_CONFIG.MIN_SPLIT_RATIO;
  const effectiveMax = maxRatio ?? LAYOUT_CONFIG.MAX_SPLIT_RATIO;
  const clampToBounds = useCallback(
    (r: number) => Math.max(effectiveMin, Math.min(effectiveMax, r)),
    [effectiveMin, effectiveMax],
  );

  // Handle mouse down
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
    startPosRef.current = direction === 'horizontal' ? e.clientX : e.clientY;
    startRatioRef.current = ratio;
  }, [direction, ratio]);

  // Handle mouse move
  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      if (!containerRef.current) return;

      const containerRect = containerRef.current.getBoundingClientRect();
      const containerSize = direction === 'horizontal' 
        ? containerRect.width 
        : containerRect.height;
      
      const currentPos = direction === 'horizontal' ? e.clientX : e.clientY;
      const delta = currentPos - startPosRef.current;
      const deltaRatio = delta / containerSize;

      const newRatio = clampToBounds(startRatioRef.current + deltaRatio);
      onRatioChange(newRatio);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, direction, containerRef, onRatioChange, clampToBounds]);

  // Double-click to reset. grid9 passes resetRatio = 1/N (N = axis count) so a
  // double-click restores an even share rather than the legacy 0.5 default.
  const handleDoubleClick = useCallback(() => {
    onRatioChange(resetRatio ?? LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO);
  }, [onRatioChange, resetRatio]);

  // Handle keyboard adjustments
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    const step = e.shiftKey ? 0.1 : 0.02;
    
    if (direction === 'horizontal') {
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        onRatioChange(clampToBounds(ratio - step));
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        onRatioChange(clampToBounds(ratio + step));
      }
    } else {
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        onRatioChange(clampToBounds(ratio - step));
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        onRatioChange(clampToBounds(ratio + step));
      }
    }
  }, [direction, ratio, onRatioChange, clampToBounds]);

  return (
    <Tooltip content={t('canvas.dragToResize')}>
      <div data-bf-component="content-canvas" data-bf-part="splitHandle" data-bf-direction={direction} data-bf-state={isDragging ? 'dragging' : ''}
        className={`canvas-split-handle canvas-split-handle--${direction} ${
          isDragging ? 'is-dragging' : ''
        }`}
        style={style}
        onMouseDown={handleMouseDown}
        onDoubleClick={handleDoubleClick}
        onKeyDown={handleKeyDown}
        tabIndex={0}
        role="separator"
        aria-orientation={direction}
        aria-valuenow={Math.round(ratio * 100)}
        aria-valuemin={LAYOUT_CONFIG.MIN_SPLIT_RATIO * 100}
        aria-valuemax={LAYOUT_CONFIG.MAX_SPLIT_RATIO * 100}
      >
        <div className="canvas-split-handle__line" />
        <div className="canvas-split-handle__grip">
          {direction === 'horizontal' ? (
            <svg width="6" height="16" viewBox="0 0 6 16" fill="none">
              <circle cx="3" cy="4" r="1" fill="currentColor" />
              <circle cx="3" cy="8" r="1" fill="currentColor" />
              <circle cx="3" cy="12" r="1" fill="currentColor" />
            </svg>
          ) : (
            <svg width="16" height="6" viewBox="0 0 16 6" fill="none">
              <circle cx="4" cy="3" r="1" fill="currentColor" />
              <circle cx="8" cy="3" r="1" fill="currentColor" />
              <circle cx="12" cy="3" r="1" fill="currentColor" />
            </svg>
          )}
        </div>
      </div>
    </Tooltip>
  );
};

SplitHandle.displayName = 'SplitHandle';

export default SplitHandle;

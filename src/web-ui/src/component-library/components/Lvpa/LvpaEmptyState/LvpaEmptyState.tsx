/**
 * LvpaEmptyState — 空状态占位组件
 * 修仙骨架场景统一使用此组件展示空状态，避免 6 个场景各自内联实现。
 */

import React from 'react';
import './LvpaEmptyState.scss';

export interface LvpaEmptyStateProps {
  /** 世界观标题（如"宗门尚未建立"） */
  title: string;
  /** 详细描述 */
  description: string;
  /** 建议动作文案 */
  actionLabel?: string;
  /** 建议动作回调 */
  onAction?: () => void;
  /** CSS class */
  className?: string;
}

export const LvpaEmptyState: React.FC<LvpaEmptyStateProps> = ({
  title,
  description,
  actionLabel,
  onAction,
  className = '',
}) => {
  const classNames = ['lvpa-empty-state', className].filter(Boolean).join(' ');

  return (
    <div className={classNames}>
      <div className="lvpa-empty-state__icon" aria-hidden="true">
        {/* 修仙风格装饰圆 — 由 CSS 绘制 */}
      </div>
      <h3 className="lvpa-empty-state__title">{title}</h3>
      <p className="lvpa-empty-state__description">{description}</p>
      {actionLabel && onAction && (
        <button
          className="lvpa-empty-state__action"
          onClick={onAction}
          type="button"
        >
          {actionLabel}
        </button>
      )}
    </div>
  );
};

LvpaEmptyState.displayName = 'LvpaEmptyState';

/**
 * LvpaEmptyState — LVPA 修仙场景空状态占位组件
 *
 * 所有骨架场景统一使用此组件展示空状态。
 */

import React from 'react';
import './LvpaEmptyState.scss';

export interface LvpaEmptyStateProps {
  /** 世界观标题（如"宗门尚未建立"） */
  title: string;
  /** 详细描述 */
  description: string;
  /** 建议动作标签（可选） */
  actionLabel?: string;
  /** 建议动作回调（可选） */
  onAction?: () => void;
}

export const LvpaEmptyState: React.FC<LvpaEmptyStateProps> = ({
  title,
  description,
  actionLabel,
  onAction,
}) => {
  return (
    <div className="lvpa-empty-state">
      <div className="lvpa-empty-state__icon" aria-hidden="true" />
      <h2 className="lvpa-empty-state__title">{title}</h2>
      <p className="lvpa-empty-state__description">{description}</p>
      {actionLabel && onAction && (
        <button className="lvpa-empty-state__action" onClick={onAction} type="button">
          {actionLabel}
        </button>
      )}
    </div>
  );
};

export default LvpaEmptyState;

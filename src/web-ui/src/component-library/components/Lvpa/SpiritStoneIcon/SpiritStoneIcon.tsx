/**
 * SpiritStoneIcon — 灵石图标组件
 * 金币风格图标 + 数字格式化（千位分隔，sm 尺寸缩写为 1k）。
 */

import React, { useMemo } from 'react';
import './SpiritStoneIcon.scss';

export interface SpiritStoneIconProps {
  /** 灵石数量 */
  amount: number;
  /** 尺寸 */
  size?: 'sm' | 'md' | 'lg';
  /** 是否显示"灵石"文字后缀 */
  showLabel?: boolean;
  /** CSS class */
  className?: string;
}

/**
 * 格式化灵石数量：
 * - sm 尺寸：≥1000 时缩写为 1k/1.2k 等
 * - 其他尺寸：千位分隔（1,234,567）
 */
function formatAmount(amount: number, size: 'sm' | 'md' | 'lg'): string {
  if (size === 'sm' && Math.abs(amount) >= 1000) {
    const abbreviated = (amount / 1000).toFixed(amount % 1000 === 0 ? 0 : 1);
    return `${abbreviated}k`;
  }
  return amount.toLocaleString('en-US');
}

export const SpiritStoneIcon: React.FC<SpiritStoneIconProps> = ({
  amount,
  size = 'md',
  showLabel = true,
  className = '',
}) => {
  const formatted = useMemo(() => formatAmount(amount, size), [amount, size]);

  const classNames = [
    'spirit-stone-icon',
    `spirit-stone-icon--${size}`,
    className,
  ].filter(Boolean).join(' ');

  return (
    <span className={classNames} title={`${amount.toLocaleString('en-US')} 灵石`}>
      <svg
        className="spirit-stone-icon__coin"
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        aria-hidden="true"
        width="1em"
        height="1em"
      >
        {/* 外圆 */}
        <circle cx="12" cy="12" r="10" fill="var(--lvpa-gold, #d4a843)" opacity="0.25" />
        {/* 内圆 */}
        <circle cx="12" cy="12" r="7" fill="var(--lvpa-gold, #d4a843)" opacity="0.5" />
        {/* 中心菱形 — 灵石纹路 */}
        <path
          d="M12 7L14.5 12L12 17L9.5 12L12 7Z"
          fill="var(--lvpa-gold, #d4a843)"
        />
      </svg>
      <span className="spirit-stone-icon__amount">{formatted}</span>
      {showLabel && <span className="spirit-stone-icon__label">灵石</span>}
    </span>
  );
};

SpiritStoneIcon.displayName = 'SpiritStoneIcon';

/**
 * RealmBadge — 境界徽章组件
 * 根据境界名映射境界色，圆形徽章 + 境界名称，hover 时显示全称。
 */

import React, { useMemo } from 'react';
import './RealmBadge.scss';

export interface RealmBadgeProps {
  /** 境界名称 */
  realm: string;
  /** 境界色值（可选，由组件自动映射色值） */
  color?: string;
  /** 尺寸 */
  size?: 'sm' | 'md' | 'lg';
  /** 是否显示境界名称 */
  showLabel?: boolean;
  /** CSS class */
  className?: string;
}

/** 7 境界色映射表 */
const REALM_COLOR_MAP: Record<string, string> = {
  '炼气': '#8a8a8a',
  '筑基': '#6d8a5e',
  '金丹': '#d4a843',
  '元婴': '#5e8ab5',
  '化神': '#b55ea8',
  '炼虚': '#8a5eb5',
  '飞升': '#5eb5a8',
  // English aliases for type safety
  qiRefining: '#8a8a8a',
  foundation: '#6d8a5e',
  goldenCore: '#d4a843',
  nascentSoul: '#5e8ab5',
  divineTransformation: '#b55ea8',
  voidRefining: '#8a5eb5',
  ascension: '#5eb5a8',
};

/** 境界全称映射（hover 显示） */
const REALM_FULL_NAME: Record<string, string> = {
  '炼气': '炼气期',
  '筑基': '筑基期',
  '金丹': '金丹期',
  '元婴': '元婴期',
  '化神': '化神期',
  '炼虚': '炼虚期',
  '飞升': '飞升期',
};

function getRealmColor(realm: string): string {
  return REALM_COLOR_MAP[realm] ?? REALM_COLOR_MAP[realm.toLowerCase()] ?? '';
}

function getRealmFullName(realm: string): string {
  return REALM_FULL_NAME[realm] ?? realm;
}

export const RealmBadge: React.FC<RealmBadgeProps> = ({
  realm,
  color,
  size = 'md',
  showLabel = true,
  className = '',
}) => {
  const resolvedColor = useMemo(() => color ?? getRealmColor(realm), [color, realm]);
  const fullName = useMemo(() => getRealmFullName(realm), [realm]);

  const classNames = [
    'realm-badge',
    `realm-badge--${size}`,
    className,
  ].filter(Boolean).join(' ');

  return (
    <span
      className={classNames}
      style={{ '--realm-color': resolvedColor } as React.CSSProperties}
      title={fullName}
    >
      <span className="realm-badge__dot" aria-hidden="true" />
      {showLabel && <span className="realm-badge__label">{realm}</span>}
    </span>
  );
};

RealmBadge.displayName = 'RealmBadge';

/**
 * SectMap — 宗门大地图组件
 *
 * 参考 agent-town 的区域叠加（zone overlay）概念，使用纯 CSS/SVG 实现。
 * 每个宗门建筑是一个可点击区域，状态颜色区分活跃/空闲/繁忙。
 */

import React, { useState, useCallback } from 'react';
import { MOCK_SECT_BUILDINGS } from './lvpa-mock-data';
import type { SectBuilding } from './lvpa-types';
import './SectMap.scss';

const GRID_COLS = 12;
const GRID_ROWS = 10;

function toPct(col: number, total: number): string {
  return `${(col / total) * 100}%`;
}

/** 区域状态中文标签 */
const STATUS_LABEL: Record<string, string> = {
  idle: '清闲',
  active: '进行中',
  busy: '繁忙',
};

interface BuildingBlockProps {
  building: SectBuilding;
  onEnter: (id: string) => void;
  onLeave: () => void;
}

const BuildingBlock: React.FC<BuildingBlockProps> = React.memo(({ building, onEnter, onLeave }) => {
  const { zone, unlocked, status, name, icon, description, unlockRealm } = building;

  const style: React.CSSProperties = {
    left: toPct(zone.x, GRID_COLS),
    top: toPct(zone.y, GRID_ROWS),
    width: toPct(zone.w, GRID_COLS),
    height: toPct(zone.h, GRID_ROWS),
  };

  const statusClass = unlocked
    ? `sect-map__building--${status}`
    : 'sect-map__building--locked';

  return (
    <div
      className={`sect-map__building ${statusClass}`}
      style={style}
      onMouseEnter={() => unlocked && onEnter(building.id)}
      onMouseLeave={onLeave}
      title={unlocked ? description : `需要${unlockRealm}境界解锁`}
    >
      <span className="sect-map__building-icon">{icon}</span>
      <span className="sect-map__building-name">{name}</span>
      <span className="sect-map__building-desc">{description}</span>
      {unlocked ? (
        <span className={`sect-map__building-status sect-map__building-status--${status}`}>
          {STATUS_LABEL[status] ?? status}
        </span>
      ) : (
        <>
          <span className="sect-map__lock">🔒</span>
          <span className="sect-map__lock-realm">{unlockRealm}</span>
        </>
      )}
    </div>
  );
});

BuildingBlock.displayName = 'BuildingBlock';

export const SectMap: React.FC = () => {
  const [hoveredBuilding, setHoveredBuilding] = useState<string | null>(null);

  const handleEnter = useCallback((id: string) => setHoveredBuilding(id), []);
  const handleLeave = useCallback(() => setHoveredBuilding(null), []);

  const hovered = hoveredBuilding
    ? MOCK_SECT_BUILDINGS.find((b) => b.id === hoveredBuilding)
    : null;

  return (
    <div className="sect-map">
      <h2 className="sect-map__title">太初宗宗门</h2>
      <p className="sect-map__subtitle">
        宗门建筑 = 后端模块。点击建筑进入对应功能。当前{MOCK_SECT_BUILDINGS.filter(b => b.unlocked).length}座已解锁
      </p>

      <div className="sect-map__grid">
        {MOCK_SECT_BUILDINGS.map((building) => (
          <BuildingBlock
            key={building.id}
            building={building}
            onEnter={handleEnter}
            onLeave={handleLeave}
          />
        ))}
      </div>

      {hovered && (
        <div className="sect-map__tooltip" style={{
          marginTop: 12, padding: '8px 14px',
          background: 'rgba(28,28,31,0.06)', borderRadius: 8,
          fontSize: 13, color: 'var(--lvpa-ink, #1c1c1f)',
        }}>
          <strong>{hovered.name}</strong>：{hovered.description}
        </div>
      )}
    </div>
  );
};

export default SectMap;

/**
 * CultivatorProfile — 修士面板组件
 *
 * 展示境界、灵根、评分、灵石、属性面板。
 * 参考 react-xiuxian-game CharacterModal 的角色展示模式。
 */

import React from 'react';
import { MOCK_CULTIVATOR } from './lvpa-mock-data';
import './CultivatorProfile.scss';

const STAT_LABELS: Record<string, string> = {
  attack: '攻击',
  defense: '防御',
  hp: '气血',
  spirit: '神识',
  speed: '速度',
};

export const CultivatorProfile: React.FC = () => {
  const c = MOCK_CULTIVATOR;

  return (
    <div className="cultivator-profile">
      {/* 头部：头像 + 基本信息 */}
      <div className="cultivator-profile__header">
        <div className="cultivator-profile__avatar">{'🧑‍⚕️'}</div>
        <div className="cultivator-profile__info">
          <h2 className="cultivator-profile__name">{c.name}</h2>
          <p className="cultivator-profile__title">{c.title}</p>
          <div className="cultivator-profile__tags">
            <span className="cultivator-profile__tag cultivator-profile__tag--realm">
              {c.realm}期
            </span>
            <span className="cultivator-profile__tag cultivator-profile__tag--root">
              {c.spiritRoot}灵根
            </span>
            <span className="cultivator-profile__tag cultivator-profile__tag--credit">
              评分 {c.credit}
            </span>
            <span className="cultivator-profile__tag cultivator-profile__tag--stones">
              💰 {c.spiritStones.toLocaleString()}
            </span>
          </div>
        </div>
      </div>

      {/* 五行属性 */}
      <div className="cultivator-profile__stats">
        <h3 className="cultivator-profile__stats-title">修为属性</h3>
        <div className="cultivator-profile__stats-grid">
          {Object.entries(c.stats).map(([key, value]) => (
            <div key={key} className="cultivator-profile__stat-item">
              <div className="cultivator-profile__stat-label">
                {STAT_LABELS[key] ?? key}
              </div>
              <div className="cultivator-profile__stat-value">{value}</div>
            </div>
          ))}
        </div>
      </div>

      {/* 当前任务 */}
      {c.currentTask && (
        <div className="cultivator-profile__task">
          <span className="cultivator-profile__task-icon">📋</span>
          <div>
            <div className="cultivator-profile__task-label">当前任务</div>
            <div className="cultivator-profile__task-name">{c.currentTask}</div>
          </div>
        </div>
      )}
    </div>
  );
};

export default CultivatorProfile;

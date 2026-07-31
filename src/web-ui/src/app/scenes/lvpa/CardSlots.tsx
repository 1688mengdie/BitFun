/**
 * CardSlots — 卡槽系统组件
 *
 * 显示本命魂卡 + 3~5 普通卡槽（部分锁定需灵石/境界解锁）。
 * 装备卡片时展示名称、品级、效果。
 * 参考 react-xiuxian-game EquipmentPanel + 架构总纲 §0.31 (#16~18)。
 */

import React, { useState, useCallback } from 'react';
import { MOCK_CARD_SLOTS } from './lvpa-mock-data';
import './CardSlots.scss';

const TYPE_LABEL: Record<string, string> = {
  natal: '本命',
  active: '主动',
  passive: '被动',
  treasure: '法宝',
};

export const CardSlots: React.FC = () => {
  // 仅演示：点击装备的卡片切换卸下/装上状态
  // 真实实现对接 TransportClient 的卡片管理接口
  const [slots, setSlots] = useState(() => MOCK_CARD_SLOTS.map(s => ({ ...s })));

  const handleUnequip = useCallback((index: number) => {
    setSlots(prev => prev.map((s, i) =>
      i === index && !s.locked ? { ...s, card: null } : s
    ));
  }, []);

  const totalEquipped = slots.filter(s => s.card !== null && !s.locked).length;

  return (
    <div className="card-slots">
      <h3 className="card-slots__title">
        卡槽系统
        <span className="card-slots__hint">
          ({totalEquipped}/{slots.filter(s => !s.locked).length} 已装备)
        </span>
      </h3>

      <div className="card-slots__grid">
        {slots.map((slot) => {
          const isNatal = slot.index === 0 && slot.card?.type === 'natal';

          if (slot.locked) {
            return (
              <div key={slot.index} className="card-slots__slot card-slots__slot--locked">
                <span className="card-slots__slot-index">#{slot.index + 1}</span>
                <span className="card-slots__lock-icon">🔒</span>
                <span className="card-slots__slot-label">{slot.label}</span>
                {slot.unlockRealm && (
                  <span className="card-slots__lock-realm">{slot.unlockRealm}解锁</span>
                )}
                {slot.unlockCost && (
                  <span className="card-slots__lock-cost">💰 {slot.unlockCost.toLocaleString()}</span>
                )}
              </div>
            );
          }

          if (!slot.card) {
            return (
              <div key={slot.index} className="card-slots__slot">
                <span className="card-slots__slot-index">#{slot.index + 1}</span>
                <span className="card-slots__slot-label">{slot.label}</span>
                <span style={{ fontSize: 20, color: '#aaa' }}>—</span>
                <span style={{ fontSize: 10, color: '#aaa' }}>空</span>
              </div>
            );
          }

          return (
            <div
              key={slot.index}
              className={`card-slots__slot ${isNatal ? 'card-slots__slot--natal' : 'card-slots__slot--filled'}`}
            >
              <span className="card-slots__slot-index">#{slot.index + 1}</span>
              {isNatal && <span className="card-slots__slot-label">本命</span>}
              <span className="card-slots__card-icon">{slot.card.icon}</span>
              <span className="card-slots__card-name">{slot.card.name}</span>
              {isNatal && <span className="card-slots__card-type">{TYPE_LABEL[slot.card.type]}</span>}
              <span className="card-slots__card-grade">{slot.card.grade}品</span>
              <span className="card-slots__card-effect">{slot.card.effect}</span>
              {!isNatal && (
                <button
                  className="card-slots__unequip"
                  onClick={() => handleUnequip(slot.index)}
                  type="button"
                  style={{
                    marginTop: 4, padding: '2px 8px',
                    fontSize: 10, borderRadius: 4,
                    border: '1px solid rgba(200,16,46,0.2)',
                    background: 'rgba(200,16,46,0.06)',
                    color: 'var(--lvpa-vermillion, #c8102e)',
                    cursor: 'pointer',
                  }}
                >
                  卸下
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default CardSlots;

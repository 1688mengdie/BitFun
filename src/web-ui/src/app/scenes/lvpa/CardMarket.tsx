/**
 * CardMarket — 坊市货架组件
 *
 * 卡片交易市场，聚焦版税经济模式：
 * - 每张卡有限复制次数（totalCopies）
 * - 卖方收取版税（royaltyPct）
 * - 越早入手收益越大，人手一套不值钱
 *
 * 参考架构总纲 §0.31 (#26) 坊市 = 版税经济
 */

import React, { useState, useCallback } from 'react';
import { MOCK_MARKET_CARDS } from './lvpa-mock-data';
import './CardMarket.scss';

const TYPE_LABEL: Record<string, string> = {
  active: '主动技',
  passive: '被动技',
  treasure: '法宝',
  natal: '本命',
};

export const CardMarket: React.FC = () => {
  const [purchased, setPurchased] = useState<Set<string>>(new Set());

  const handleBuy = useCallback((cardId: string) => {
    // 演示：点击购买标记已购（真实实现对接 TransportClient）
    setPurchased(prev => {
      const next = new Set(prev);
      next.add(cardId);
      return next;
    });
  }, []);

  return (
    <div className="card-market">
      <div className="card-market__header">
        <h2 className="card-market__title">坊市</h2>
        <p className="card-market__subtitle">
          卡片交易市场。每张卡片有限复制，卖方收取版税——越早入手收益越大
        </p>
      </div>

      <div className="card-market__grid">
        {MOCK_MARKET_CARDS.map((card) => {
          const bought = purchased.has(card.id);
          const copiesLeft = card.totalCopies - card.copiesSold;
          const soldOut = copiesLeft <= 0;

          return (
            <div key={card.id} className="card-market__card">
              {/* 头部 */}
              <div className="card-market__card-icon-row">
                <span className="card-market__card-icon">📜</span>
                <div className="card-market__card-info">
                  <div className="card-market__card-name">{card.name}</div>
                  <div className="card-market__card-tags">
                    <span className="card-market__card-tag">{card.grade}品</span>
                    <span className="card-market__card-tag">{TYPE_LABEL[card.type]}</span>
                    <span className="card-market__card-tag">{card.realmLock}可用</span>
                  </div>
                </div>
              </div>

              {/* 描述 */}
              <div className="card-market__card-desc">{card.description}</div>
              <div className="card-market__card-effect">{card.effect}</div>

              {/* 经济信息（版税展示） */}
              <div className="card-market__card-economy">
                <div className="card-market__economy-item">
                  <div className="card-market__economy-label">售价</div>
                  <div className="card-market__economy-value">
                    💰 {card.price.toLocaleString()}
                  </div>
                  <div className="card-market__economy-sub">灵石</div>
                </div>
                <div className="card-market__economy-item">
                  <div className="card-market__economy-label">版税率</div>
                  <div className="card-market__economy-value">{card.royaltyPct}%</div>
                  <div className="card-market__economy-sub">卖方抽成</div>
                </div>
                <div className="card-market__economy-item">
                  <div className="card-market__economy-label">已售</div>
                  <div className="card-market__economy-value">
                    {card.copiesSold}/{card.totalCopies}
                  </div>
                  <div className="card-market__economy-sub">份</div>
                </div>
                <div className="card-market__economy-item">
                  <div className="card-market__economy-label">剩余</div>
                  <div className="card-market__economy-value" style={{
                    color: copiesLeft < 10 ? 'var(--lvpa-vermillion, #c8102e)' : undefined,
                  }}>
                    {copiesLeft > 0 ? copiesLeft : '售罄'}
                  </div>
                  <div className="card-market__economy-sub">
                    {copiesLeft > 0 ? '份' : ''}
                  </div>
                </div>
              </div>

              {/* 底部：作者 + 购买 */}
              <div className="card-market__card-footer">
                <span className="card-market__card-author">
                  作者：<strong>{card.author}</strong>
                </span>
                <button
                  className="card-market__card-buy"
                  disabled={bought || soldOut}
                  onClick={() => handleBuy(card.id)}
                  type="button"
                >
                  {bought ? '已拥有' : soldOut ? '售罄' : '购买'}
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default CardMarket;

/**
 * PartyLobby — 副本大厅组件
 *
 * 副本（项目组队）大厅：
 * - 展示开放/进行中/已关闭的副本
 * - 每位副本显示组队成员列表及就绪状态
 * - "加入队伍"按钮（演示用）
 *
 * 参考架构总纲 §0.31 (#29) 副本 = 项目组队
 */

import React, { useState, useCallback } from 'react';
import { MOCK_DUNGEONS, MOCK_PARTY_MEMBERS } from './lvpa-mock-data';
import './PartyLobby.scss';

const STATUS_LABEL: Record<string, string> = {
  open: '招募中',
  in_progress: '进行中',
  closed: '已关闭',
};

const DUNGEON_ICONS = ['⚔', '🛡', '🔮', '💫'];

export const PartyLobby: React.FC = () => {
  const [joinedDungeons, setJoinedDungeons] = useState<Set<string>>(new Set());

  const handleJoin = useCallback((dungeonId: string) => {
    setJoinedDungeons((prev) => {
      const next = new Set(prev);
      if (next.has(dungeonId)) next.delete(dungeonId);
      else next.add(dungeonId);
      return next;
    });
  }, []);

  return (
    <div className="party-lobby">
      <div className="party-lobby__header">
        <h2 className="party-lobby__title">副本大厅</h2>
        <p className="party-lobby__subtitle">
          副本 = 项目组队。临时组队执行任务，完成后领赏散伙
        </p>
      </div>

      <div className="party-lobby__grid">
        {MOCK_DUNGEONS.map((dungeon, idx) => {
          const joined = joinedDungeons.has(dungeon.id);
          const spotsLeft = dungeon.partySize - dungeon.currentMembers;

          return (
            <div
              key={dungeon.id}
              className={`party-lobby__dungeon party-lobby__dungeon--${dungeon.status}`}
            >
              {/* 头部 */}
              <div className="party-lobby__dungeon-top">
                <span className="party-lobby__dungeon-icon">
                  {DUNGEON_ICONS[idx % DUNGEON_ICONS.length]}
                </span>
                <div className="party-lobby__dungeon-info">
                  <div className="party-lobby__dungeon-name">{dungeon.name}</div>
                  <div className="party-lobby__dungeon-desc">{dungeon.description}</div>

                  <div className="party-lobby__dungeon-meta">
                    <span className="party-lobby__dungeon-meta-item">
                      🏔 {dungeon.requiredRealm}
                    </span>
                    <span className="party-lobby__dungeon-meta-item">
                      👥 {dungeon.currentMembers}/{dungeon.partySize}人
                    </span>
                    {spotsLeft > 0 && (
                      <span className="party-lobby__dungeon-meta-item" style={{ color: 'var(--lvpa-jade, #7eb09b)' }}>
                        缺{spotsLeft}人
                      </span>
                    )}
                    <span className={`party-lobby__status-badge party-lobby__status-badge--${dungeon.status}`}>
                      {STATUS_LABEL[dungeon.status]}
                    </span>
                  </div>
                </div>
              </div>

              {/* 奖励 */}
              <div className="party-lobby__dungeon-rewards">
                🎁 {dungeon.rewards}
              </div>

              {/* 组队成员 */}
              {dungeon.status !== 'closed' && (
                <div className="party-lobby__party">
                  {MOCK_PARTY_MEMBERS.filter((_, i) => i < dungeon.currentMembers).map((member) => (
                    <div
                      key={member.id}
                      className={`party-lobby__member ${
                        member.ready
                          ? 'party-lobby__member--ready'
                          : 'party-lobby__member--not-ready'
                      }`}
                    >
                      <span className="party-lobby__member-name">{member.name}</span>
                      <span className="party-lobby__member-realm">({member.realm})</span>
                      <span className="party-lobby__member-role">· {member.role}</span>
                      <span
                        className={`party-lobby__member-status ${
                          !member.ready ? 'party-lobby__member-status--not-ready' : ''
                        }`}
                      >
                        {member.ready ? '就绪' : '准备中'}
                      </span>
                    </div>
                  ))}

                  {/* 加入/离开按钮 */}
                  {dungeon.status === 'open' && (
                    <button
                      className="party-lobby__join-btn"
                      onClick={() => handleJoin(dungeon.id)}
                      type="button"
                      disabled={!joined && spotsLeft <= 0}
                    >
                      {joined ? '退出队伍' : spotsLeft > 0 ? '加入队伍' : '已满'}
                    </button>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default PartyLobby;

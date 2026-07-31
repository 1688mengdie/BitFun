import React, { useState, useMemo } from 'react';
import './LibraryScene.scss';

/** 藏经阁典籍分类 */
const SECTIONS = [
  {
    id: 'theory',
    icon: '📜',
    title: '量价时空理论',
    items: [
      { name: '四总纲导论', desc: '量价时空理论体系总览', meta: 'v14' },
      { name: '双线多维分析法', desc: '双线状态 × 多维确认的完整框架', meta: '核心' },
      { name: '五阶段定位法', desc: '左/极左/轴心/极右/右——趋势定位五阶段', meta: '核心' },
      { name: 'DVMI 趋势线引擎 v3.0', desc: '动态多时间框架趋势线算法', meta: '进阶' },
      { name: '三推衰竭模型', desc: '三次推动后衰竭的识别与应对', meta: '进阶' },
    ],
  },
  {
    id: 'practice',
    icon: '📖',
    title: '实战心得',
    items: [
      { name: 'BTC 日线趋势识别案例', desc: '2025 年 BTC 日线级别趋势分析实操', meta: '2025' },
      { name: 'ETH 资金费率套利', desc: '永续合约资金费率策略实战记录', meta: '策略' },
      { name: '多周期共振实战', desc: '15min+1h+4h 三周期共振开仓案例', meta: '技巧' },
      { name: '风控止损体系', desc: '分层止损 + 移动止盈的完整风控方案', meta: '必修' },
    ],
  },
  {
    id: 'system',
    icon: '⚙',
    title: '系统功法',
    items: [
      { name: 'LVPA 架构总纲', desc: '三层架构、12 基础设施、3 业务子系统', meta: 'v2.4' },
      { name: '技术总纲', desc: 'Rust/TS 技术选型与工程规范', meta: 'v7.0' },
      { name: '量化总纲', desc: '因子、回测、实盘全链路量化规范', meta: 'v14.2' },
      { name: '工坊操作手册', desc: '天机坊/金算坊/丹青坊/留影坊使用指南', meta: '指南' },
    ],
  },
  {
    id: 'card',
    icon: '🃏',
    title: '卡片图鉴',
    items: [
      { name: '本命魂卡总览', desc: '全本命魂卡一览及属性对照', meta: '21张' },
      { name: '套装效果查询', desc: '卡片套装组合与特效说明书', meta: '12套' },
      { name: '境界锁定规则', desc: '各境界可用卡片清单', meta: '规则' },
    ],
  },
];

export const LibraryScene: React.FC<{ isActive?: boolean }> = () => {
  const [search, setSearch] = useState('');
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(['theory']));

  const toggleSection = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const filteredSections = useMemo(() => {
    if (!search.trim()) return SECTIONS;
    const q = search.toLowerCase();
    return SECTIONS.map((sec) => ({
      ...sec,
      items: sec.items.filter(
        (item) =>
          item.name.toLowerCase().includes(q) ||
          item.desc.toLowerCase().includes(q),
      ),
    })).filter((sec) => sec.items.length > 0);
  }, [search]);

  return (
    <div className="library-scene">
      <div className="library-scene__header">
        <h2 className="library-scene__title">藏经阁</h2>
        <p className="library-scene__subtitle">
          万千功法典籍汇聚之所。搜索、查阅量价时空理论、交易心得、系统功法
        </p>
      </div>

      <input
        className="library-scene__search"
        type="text"
        placeholder="搜索功法典籍…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
      />

      <div className="library-scene__sections">
        {filteredSections.map((section) => (
          <div key={section.id} className="library-scene__section">
            <div
              className="library-scene__section-header"
              onClick={() => toggleSection(section.id)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => e.key === 'Enter' && toggleSection(section.id)}
            >
              <span className="library-scene__section-icon">{section.icon}</span>
              <span className="library-scene__section-title">{section.title}</span>
              <span className="library-scene__section-count">{section.items.length}篇</span>
            </div>

            {expanded.has(section.id) && (
              <div className="library-scene__section-body">
                {section.items.map((item) => (
                  <div key={item.name} className="library-scene__item">
                    <span className="library-scene__item-icon">{'📄'}</span>
                    <div className="library-scene__item-info">
                      <div className="library-scene__item-name">{item.name}</div>
                      <div className="library-scene__item-desc">{item.desc}</div>
                    </div>
                    <span className="library-scene__item-meta">{item.meta}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};

export default LibraryScene;

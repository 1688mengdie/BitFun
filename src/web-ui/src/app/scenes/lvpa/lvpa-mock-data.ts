/**
 * lvpa-mock-data — LVPA 修仙 UI 前端 Mock 数据
 *
 * 后端就绪后替换为 TransportClient 真实调用即可。
 * 所有数据格式与 lvpa-types 类型完全对齐。
 */

import type {
  CultivatorInfo,
  CardDef,
  CardSlotDef,
  SectBuilding,
  WorkshopDef,
  DagNode,
  DagEdge,
  MarketCard,
  DungeonDef,
  PartyMember,
} from './lvpa-types';

// ── 当前修士信息 ──

export const MOCK_CULTIVATOR: CultivatorInfo = {
  id: 'a01',
  name: '散修·无名',
  realm: '筑基',
  spiritRoot: '火',
  credit: 78,
  spiritStones: 12450,
  level: 23,
  title: '初入道途',
  joinedAt: Date.now() - 86400_000 * 30,
  currentTask: '分析 BTC 日线趋势',
  stats: { attack: 45, defense: 32, hp: 280, spirit: 68, speed: 21 },
};

// ── 卡片定义 ──

const ALL_CARDS: CardDef[] = [
  { id: 'natal-1', name: '量价之眼', type: 'natal', description: '看穿成交量与价格的本质关系，识破主力意图', realmLock: '炼气', grade: '灵', effect: '成交量异动识别 +15%', icon: '👁', setGroup: '量价套装' },
  { id: 'card-1', name: '趋势之尺', type: 'active', description: '精确测量趋势力度与背离', realmLock: '筑基', grade: '玄', effect: '趋势力度量化 +20%', icon: '📏', setGroup: '量价套装' },
  { id: 'card-2', name: '时空之门', type: 'active', description: '多周期共振分析', realmLock: '金丹', grade: '地', effect: '多周期信号置信度 +25%', icon: '🚪' },
  { id: 'card-3', name: '金甲护体', type: 'passive', description: '自动止盈止损保护', realmLock: '炼气', grade: '凡', effect: '最大回撤 -30%', icon: '🛡' },
  { id: 'card-4', name: '灵石口袋', type: 'passive', description: '交易手续费折扣', realmLock: '筑基', grade: '灵', effect: '手续费 -15%', icon: '💰' },
  { id: 'card-5', name: '天机罗盘', type: 'treasure', description: '预判关键转折点', realmLock: '元婴', grade: '天', effect: '转折点预警 +35%', icon: '🧭' },
];

export function getMockCards(): CardDef[] {
  return ALL_CARDS;
}

// ── 卡槽状态 ──

export const MOCK_CARD_SLOTS: CardSlotDef[] = [
  { index: 0, label: '本命魂卡', card: ALL_CARDS[0], locked: false },
  { index: 1, label: '卡槽一', card: ALL_CARDS[1], locked: false },
  { index: 2, label: '卡槽二', card: ALL_CARDS[3], locked: false },
  { index: 3, label: '卡槽三', card: null, locked: true, unlockRealm: '金丹', unlockCost: 50000 },
  { index: 4, label: '卡槽四', card: null, locked: true, unlockRealm: '元婴', unlockCost: 200000 },
];

// ── 宗门建筑 ──

export const MOCK_SECT_BUILDINGS: SectBuilding[] = [
  { id: 'hall', name: '任务堂', description: '领取宗门任务，赚取灵石与贡献', icon: '📜', zone: { x: 3, y: 1, w: 3, h: 2 }, unlocked: true, status: 'active' },
  { id: 'equip', name: '装备堂', description: '卡片装备与管理', icon: '⚔', zone: { x: 7, y: 1, w: 3, h: 2 }, unlocked: true, status: 'idle' },
  { id: 'awaken', name: '启灵堂', description: '本命魂卡觉醒与灵根测定', icon: '✨', zone: { x: 1, y: 4, w: 2, h: 2 }, unlocked: true, status: 'busy' },
  { id: 'library', name: '藏经阁', description: '查阅功法典籍、交易心得', icon: '📚', zone: { x: 4, y: 4, w: 2, h: 2 }, unlocked: true, status: 'idle' },
  { id: 'divine', name: '天机阁', description: '洞察天机，总结规律', icon: '🔮', zone: { x: 7, y: 4, w: 2, h: 2 }, unlocked: true, status: 'idle' },
  { id: 'cave', name: '洞府', description: '专属修炼洞府', icon: '🏠', zone: { x: 1, y: 7, w: 2, h: 2 }, unlocked: true, status: 'active' },
  { id: 'merit', name: '功德堂', description: '评分、审计与晋升', icon: '🏆', zone: { x: 4, y: 7, w: 2, h: 2 }, unlocked: true, status: 'idle' },
  { id: 'gate', name: '山门', description: '接入外部网络', icon: '🚪', zone: { x: 7, y: 7, w: 3, h: 2 }, unlocked: true, status: 'active' },
];

// ── 工坊 ──

export const MOCK_WORKSHOPS: WorkshopDef[] = [
  { id: 'tianji', name: 'Tianji Forge', nameCN: '天机坊', description: '代码开发工坊——铸造功法、炼制法宝', icon: '⚙', memberCount: 3, status: 'running', currentProject: 'Phase 6 UI', progressPct: 65 },
  { id: 'jinsuan', name: 'Jinsuan Hall', nameCN: '金算坊', description: '交易量化工坊——操盘、回测、策略', icon: '📊', memberCount: 2, status: 'running', currentProject: 'BTC 趋势策略', progressPct: 42 },
  { id: 'danqing', name: 'Danqing Studio', nameCN: '丹青坊', description: '美术设计工坊——UI、图表、视觉', icon: '🎨', memberCount: 1, status: 'paused', currentProject: '修仙主题图标', progressPct: 80 },
  { id: 'liuying', name: 'Liuying Studio', nameCN: '留影坊', description: '视频内容工坊——录制、剪辑、直播', icon: '🎬', memberCount: 1, status: 'idle' },
];

export const MOCK_WORKSHOP_DAG: Record<string, { nodes: DagNode[]; edges: DagEdge[] }> = {
  tianji: {
    nodes: [
      { id: 'req', label: '需求分析', type: 'input', status: 'done' },
      { id: 'design', label: '方案设计', type: 'process', status: 'done' },
      { id: 'impl', label: '编码实现', type: 'process', status: 'running', progress: 65 },
      { id: 'review', label: '代码审查', type: 'process', status: 'pending' },
      { id: 'test', label: '测试验证', type: 'process', status: 'pending' },
      { id: 'deploy', label: '部署交付', type: 'output', status: 'pending' },
    ],
    edges: [
      { from: 'req', to: 'design' },
      { from: 'design', to: 'impl' },
      { from: 'impl', to: 'review' },
      { from: 'review', to: 'test' },
      { from: 'test', to: 'deploy' },
    ],
  },
  jinsuan: {
    nodes: [
      { id: 'data', label: '数据获取', type: 'input', status: 'done' },
      { id: 'backtest', label: '回测验证', type: 'process', status: 'running', progress: 42 },
      { id: 'optimize', label: '参数优化', type: 'process', status: 'pending' },
      { id: 'forward', label: '前瞻测试', type: 'process', status: 'pending' },
      { id: 'live', label: '实盘部署', type: 'output', status: 'pending' },
    ],
    edges: [
      { from: 'data', to: 'backtest' },
      { from: 'backtest', to: 'optimize' },
      { from: 'optimize', to: 'forward' },
      { from: 'forward', to: 'live' },
    ],
  },
};

// ── 坊市货架 ──

export const MOCK_MARKET_CARDS: MarketCard[] = [
  { id: 'm-1', name: '趋势之尺', type: 'active', grade: '玄', description: '精确测量趋势力度与背离', price: 50000, royaltyPct: 5, author: '太初宗·青云', copiesSold: 23, totalCopies: 100, realmLock: '筑基', effect: '趋势力度量化 +20%' },
  { id: 'm-2', name: '金甲护体', type: 'passive', grade: '凡', description: '自动止盈止损保护', price: 15000, royaltyPct: 3, author: '散修·铁壁', copiesSold: 89, totalCopies: 500, realmLock: '炼气', effect: '最大回撤 -30%' },
  { id: 'm-3', name: '灵石口袋', type: 'passive', grade: '灵', description: '交易手续费折扣', price: 30000, royaltyPct: 4, author: '太初宗·金算', copiesSold: 45, totalCopies: 200, realmLock: '筑基', effect: '手续费 -15%' },
  { id: 'm-4', name: '天机罗盘', type: 'treasure', grade: '天', description: '预判关键转折点', price: 200000, royaltyPct: 8, author: '天机阁主', copiesSold: 5, totalCopies: 20, realmLock: '元婴', effect: '转折点预警 +35%' },
  { id: 'm-5', name: '时空之门', type: 'active', grade: '地', description: '多周期共振分析', price: 100000, royaltyPct: 6, author: '太初宗·青云', copiesSold: 12, totalCopies: 50, realmLock: '金丹', effect: '多周期信号置信度 +25%' },
];

// ── 副本 ──

export const MOCK_DUNGEONS: DungeonDef[] = [
  { id: 'd-1', name: 'BTC 日线秘境', description: '分析 BTC 日线级别趋势，识别关键支撑阻力位', requiredRealm: '炼气', rewards: '灵石×5000, 经验×200', status: 'open', partySize: 3, currentMembers: 1 },
  { id: 'd-2', name: 'ETH 合约战场', description: 'ETH 永续合约资金费率套利策略研究', requiredRealm: '筑基', rewards: '灵石×15000, 功法碎片×3', status: 'open', partySize: 4, currentMembers: 2 },
  { id: 'd-3', name: '多币种矩阵', description: '跨交易所价差监控与套利信号捕捉', requiredRealm: '金丹', rewards: '灵石×50000, 灵·装备箱×1', status: 'in_progress', partySize: 5, currentMembers: 3 },
  { id: 'd-4', name: '量化飞升试炼', description: '全自动策略从回测到实盘的一键部署验证', requiredRealm: '元婴', rewards: '灵石×200000, 地·功法×1', status: 'closed', partySize: 6, currentMembers: 6 },
];

export const MOCK_PARTY_MEMBERS: PartyMember[] = [
  { id: 'p-1', name: '散修·无名', realm: '筑基', role: '策略分析', ready: true },
  { id: 'p-2', name: '天机·青云', realm: '金丹', role: '数据工程', ready: true },
  { id: 'p-3', name: '金算·铁壁', realm: '筑基', role: '风控', ready: false },
];

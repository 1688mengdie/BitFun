/**
 * lvpa-types — LVPA 修仙 UI 场景共享类型
 *
 * 对应 Phase-6-类型契约.md 和架构总纲 §4a。
 * 所有场景组件直接使用此处定义的类型；后端就绪后替换 mock 数据层即可。
 */

// ── 修士（Agent）信息 ──

export interface CultivatorInfo {
  id: string;
  name: string;
  realm: string;
  spiritRoot: string;
  credit: number;
  spiritStones: number;
  level: number;
  title: string;
  joinedAt: number;
  currentTask?: string;
  stats: {
    attack: number;
    defense: number;
    hp: number;
    spirit: number;
    speed: number;
  };
}

// ── 卡片系统 ──

export type CardType = 'natal' | 'active' | 'passive' | 'treasure';

export interface CardDef {
  id: string;
  name: string;
  type: CardType;
  description: string;
  realmLock: string;
  grade: '凡' | '灵' | '玄' | '地' | '天' | '仙';
  effect: string;
  icon: string;
  setBonus?: string;
  setGroup?: string;
}

export interface CardSlotDef {
  index: number;
  label: string;
  card: CardDef | null;
  locked: boolean;
  unlockRealm?: string;
  /** 开锁所需灵石 */
  unlockCost?: number;
}

// ── 宗门建筑 ──

export interface SectBuilding {
  id: string;
  name: string;
  description: string;
  icon: string;
  zone: { x: number; y: number; w: number; h: number };
  unlocked: boolean;
  unlockRealm?: string;
  status: 'idle' | 'active' | 'busy';
}

// ── 工坊 ──

export type WorkshopType = 'tianji' | 'jinsuan' | 'danqing' | 'liuying';

export interface WorkshopDef {
  id: WorkshopType;
  name: string;
  nameCN: string;
  description: string;
  icon: string;
  memberCount: number;
  status: 'running' | 'paused' | 'idle';
  currentProject?: string;
  progressPct?: number;
}

export interface DagNode {
  id: string;
  label: string;
  type: 'input' | 'process' | 'output';
  status: 'pending' | 'running' | 'done' | 'error';
  progress?: number;
}

export interface DagEdge {
  from: string;
  to: string;
}

// ── 坊市卡片 ──

export interface MarketCard {
  id: string;
  name: string;
  type: CardType;
  grade: string;
  description: string;
  price: number;
  royaltyPct: number;
  author: string;
  copiesSold: number;
  totalCopies: number;
  realmLock: string;
  effect: string;
}

// ── 副本 ──

export interface DungeonDef {
  id: string;
  name: string;
  description: string;
  requiredRealm: string;
  rewards: string;
  status: 'open' | 'in_progress' | 'closed';
  partySize: number;
  currentMembers: number;
}

export interface PartyMember {
  id: string;
  name: string;
  realm: string;
  role: string;
  ready: boolean;
}

// ── 套装效果 ──

export interface SetBonusDef {
  id: string;
  name: string;
  cardsRequired: number;
  effect: string;
  grade: string;
}

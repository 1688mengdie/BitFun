/**
 * Agents scene state management
 */
import { create } from 'zustand';
import type { SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import type { SubagentModelSelection } from '@/infrastructure/config/types';
import {
  CAPABILITY_ACCENT,
  CAPABILITY_CATEGORIES,
  type CapabilityCategory,
} from './agentAppearance';

export { CAPABILITY_CATEGORIES };
export type { CapabilityCategory };

/** 'mode' = primary agent mode (e.g. Agentic/Plan/Debug); 'subagent' = sub-agent */
export type AgentKind = 'mode' | 'subagent';

export interface AgentCapability {
  category: CapabilityCategory;
  level: number;
}

export interface AgentWithCapabilities extends SubagentInfo {
  capabilities: AgentCapability[];
  iconKey?: string;
  /** Distinguishes primary agent mode from sub-agent */
  agentKind?: AgentKind;
  visibleSubagentCount?: number;
  /** Explicit model selection for this Subagent, if it overrides the shared default. */
  subagentModelOverride?: SubagentModelSelection;
  /** Display name for an explicitly configured Subagent model override. */
  subagentModelDisplayName?: string;
}

export const CAPABILITY_COLORS: Record<CapabilityCategory, string> = CAPABILITY_ACCENT;

// ─── Agent team model (recovered from afc8c0aa1~1, adapted to HEAD) ───────────

export type MemberRole = 'leader' | 'member' | 'reviewer';
export type AgentTeamStrategy = 'sequential' | 'collaborative' | 'free';
export type AgentTeamViewMode = 'formation' | 'list';

export interface AgentTeamMember {
  agentId: string;
  role: MemberRole;
  modelOverride?: string;
  order: number;
}

export interface AgentTeam {
  id: string;
  name: string;
  icon: string;
  description: string;
  members: AgentTeamMember[];
  strategy: AgentTeamStrategy;
  shareContext: boolean;
}

/** Mock agents used by the recovered team gallery/editor (builtin-only). */
export const MOCK_AGENT_TEAMS: AgentTeam[] = [
  {
    id: 'agent-team-coding',
    name: 'Coding Team',
    icon: 'code',
    description: 'Code review, refactoring and quality assurance',
    members: [
      { agentId: 'agentic', role: 'leader', order: 0 },
      { agentId: 'CodeReview', role: 'member', order: 1 },
      { agentId: 'Debug', role: 'member', order: 2 },
      { agentId: 'GeneralPurpose', role: 'reviewer', order: 3 },
    ],
    strategy: 'collaborative',
    shareContext: true,
  },
  {
    id: 'agent-team-research',
    name: 'Research Team',
    icon: 'chart',
    description: 'Information gathering, data analysis and report writing',
    members: [
      { agentId: 'DeepResearch', role: 'leader', order: 0 },
      { agentId: 'Explore', role: 'member', order: 1 },
      { agentId: 'FileFinder', role: 'reviewer', order: 2 },
    ],
    strategy: 'sequential',
    shareContext: true,
  },
  {
    id: 'agent-team-ppt',
    name: 'PPT Production',
    icon: 'layout',
    description: 'Content planning, visual design and copy polishing',
    members: [
      { agentId: 'Cowork', role: 'leader', order: 0 },
    ],
    strategy: 'collaborative',
    shareContext: false,
  },
];

export const AGENT_TEAM_TEMPLATES: Array<{
  id: string;
  name: string;
  icon: string;
  description: string;
  memberIds: string[];
}> = [
  {
    id: 'tpl-coding',
    name: 'Coding Team',
    icon: 'code',
    description: 'Code review, refactoring and quality assurance',
    memberIds: ['agentic', 'CodeReview', 'Debug', 'GeneralPurpose'],
  },
  {
    id: 'tpl-research',
    name: 'Research Team',
    icon: 'chart',
    description: 'Information gathering, data analysis and report writing',
    memberIds: ['DeepResearch', 'Explore', 'FileFinder'],
  },
  {
    id: 'tpl-ppt',
    name: 'PPT Production',
    icon: 'layout',
    description: 'Content planning, copy and visual planning',
    memberIds: ['Cowork'],
  },
  {
    id: 'tpl-fullstack',
    name: 'Fullstack Team',
    icon: 'rocket',
    description: 'End-to-end development, testing and documentation',
    memberIds: ['agentic', 'Debug', 'GeneralPurpose', 'CodeReview'],
  },
];

/** Compute the max capability level a team covers, keyed by capability category. */
export function computeAgentTeamCapabilities(
  team: AgentTeam,
  allAgents: AgentWithCapabilities[],
): Record<CapabilityCategory, number> {
  const result: Record<CapabilityCategory, number> = {
    coding: 0,
    docs: 0,
    analysis: 0,
    testing: 0,
    creative: 0,
    ops: 0,
  };
  for (const member of team.members) {
    const agent = allAgents.find((a) => a.id === member.agentId);
    if (!agent) continue;
    for (const cap of agent.capabilities) {
      result[cap.category] = Math.max(result[cap.category], cap.level);
    }
  }
  return result;
}

export type AgentsScenePage = 'home' | 'createAgent' | 'createLegion' | 'reviewTeam' | 'agentTeamEditor';
export type AgentEditorMode = 'create' | 'edit';
export type AgentFilterLevel = 'all' | 'builtin' | 'user' | 'project' | 'external';
export type AgentFilterType = 'all' | 'mode' | 'subagent';

interface AgentsStoreState {
  page: AgentsScenePage;
  agentEditorMode: AgentEditorMode;
  editingAgentId: string | null;
  searchQuery: string;
  agentFilterLevel: AgentFilterLevel;
  agentFilterType: AgentFilterType;
  setPage: (page: AgentsScenePage) => void;
  setSearchQuery: (query: string) => void;
  setAgentFilterLevel: (filter: AgentFilterLevel) => void;
  setAgentFilterType: (filter: AgentFilterType) => void;
  openHome: () => void;
  openCreateAgent: () => void;
  openCreateLegion: () => void;
  openEditAgent: (agentId: string) => void;
  openReviewTeam: () => void;
  openAgentTeamEditor: (teamId: string) => void;

  // Agent team editor state (recovered from afc8c0aa1~1)
  agentTeams: AgentTeam[];
  activeAgentTeamId: string | null;
  viewMode: AgentTeamViewMode;
  /** Shared agent data for the team gallery/editor, synced from useAgentsList. */
  teamComposerAgents: AgentWithCapabilities[];
  setTeamComposerAgents: (agents: AgentWithCapabilities[]) => void;
  setActiveAgentTeam: (id: string | null) => void;
  setViewMode: (mode: AgentTeamViewMode) => void;
  addAgentTeam: (team: Omit<AgentTeam, 'members'>) => void;
  updateAgentTeam: (id: string, patch: Partial<Pick<AgentTeam, 'name' | 'icon' | 'description' | 'strategy' | 'shareContext'>>) => void;
  deleteAgentTeam: (id: string) => void;
  addMember: (teamId: string, agentId: string, role?: MemberRole) => void;
  removeMember: (teamId: string, agentId: string) => void;
  updateMemberRole: (teamId: string, agentId: string, role: MemberRole) => void;
}

export const useAgentsStore = create<AgentsStoreState>((set) => ({
  page: 'home',
  agentEditorMode: 'create',
  editingAgentId: null,
  searchQuery: '',
  agentFilterLevel: 'all',
  agentFilterType: 'all',
  setPage: (page) => set({ page }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  setAgentFilterLevel: (filter) => set({ agentFilterLevel: filter }),
  setAgentFilterType: (filter) => set({ agentFilterType: filter }),
  openHome: () => set({ page: 'home', agentEditorMode: 'create', editingAgentId: null }),
  openCreateAgent: () => set({
    page: 'createAgent',
    agentEditorMode: 'create',
    editingAgentId: null,
  }),
  openCreateLegion: () => set({ page: 'createLegion' }),
  openEditAgent: (agentId: string) => set({
    page: 'createAgent',
    agentEditorMode: 'edit',
    editingAgentId: agentId,
  }),
  openReviewTeam: () => set({ page: 'reviewTeam' }),
  openAgentTeamEditor: (teamId) => set({ page: 'agentTeamEditor', activeAgentTeamId: teamId }),

  agentTeams: MOCK_AGENT_TEAMS,
  activeAgentTeamId: MOCK_AGENT_TEAMS[0].id,
  viewMode: 'formation',
  teamComposerAgents: [],
  setTeamComposerAgents: (agents) => set({ teamComposerAgents: agents }),
  setActiveAgentTeam: (id) => set({ activeAgentTeamId: id }),
  setViewMode: (mode) => set({ viewMode: mode }),
  addAgentTeam: (team) => {
    const newAgentTeam: AgentTeam = { ...team, members: [] };
    set((s) => ({ agentTeams: [...s.agentTeams, newAgentTeam], activeAgentTeamId: newAgentTeam.id }));
  },
  updateAgentTeam: (id, patch) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    })),
  deleteAgentTeam: (id) =>
    set((s) => {
      const next = s.agentTeams.filter((t) => t.id !== id);
      const activeId = s.activeAgentTeamId === id ? (next[0]?.id ?? null) : s.activeAgentTeamId;
      return { agentTeams: next, activeAgentTeamId: activeId };
    }),
  addMember: (teamId, agentId, role = 'member') =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) => {
        if (t.id !== teamId) return t;
        if (t.members.some((m) => m.agentId === agentId)) return t;
        const newMember: AgentTeamMember = { agentId, role, order: t.members.length };
        return { ...t, members: [...t.members, newMember] };
      }),
    })),
  removeMember: (teamId, agentId) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) =>
        t.id === teamId
          ? { ...t, members: t.members.filter((m) => m.agentId !== agentId) }
          : t,
      ),
    })),
  updateMemberRole: (teamId, agentId, role) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) =>
        t.id === teamId
          ? { ...t, members: t.members.map((m) => (m.agentId === agentId ? { ...m, role } : m)) }
          : t,
      ),
    })),
}));

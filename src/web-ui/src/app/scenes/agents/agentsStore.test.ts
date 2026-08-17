import { describe, expect, it } from 'vitest';
import {
  MOCK_AGENT_TEAMS,
  computeAgentTeamCapabilities,
  useAgentsStore,
} from './agentsStore';

describe('agentsStore team state (recovered by R-WF-13)', () => {
  it('seeds mock agent teams with the first team active', () => {
    const state = useAgentsStore.getState();
    expect(state.agentTeams).toHaveLength(MOCK_AGENT_TEAMS.length);
    expect(state.activeAgentTeamId).toBe(MOCK_AGENT_TEAMS[0].id);
    expect(state.viewMode).toBe('formation');
  });

  it('adds an agent team and selects it', () => {
    const { addAgentTeam } = useAgentsStore.getState();
    addAgentTeam({
      id: 'agent-team-test-1',
      name: 'Test Team',
      icon: 'rocket',
      description: '',
      strategy: 'collaborative',
      shareContext: true,
    });
    const state = useAgentsStore.getState();
    expect(state.agentTeams.some((t) => t.id === 'agent-team-test-1')).toBe(true);
    expect(state.activeAgentTeamId).toBe('agent-team-test-1');
    // cleanup
    useAgentsStore.getState().deleteAgentTeam('agent-team-test-1');
  });

  it('adds/removes members and updates roles', () => {
    const teamId = 'agent-team-coding';
    const { addMember, removeMember, updateMemberRole } = useAgentsStore.getState();
    addMember(teamId, 'agentic', 'leader');
    let team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    expect(team.members.some((m) => m.agentId === 'agentic')).toBe(true);

    updateMemberRole(teamId, 'agentic', 'reviewer');
    team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    expect(team.members.find((m) => m.agentId === 'agentic')?.role).toBe('reviewer');

    removeMember(teamId, 'agentic');
    team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    expect(team.members.some((m) => m.agentId === 'agentic')).toBe(false);
  });

  it('computes team capability coverage from member agents', () => {
    const team = MOCK_AGENT_TEAMS[0];
    const agents = [
      { id: 'agentic', capabilities: [{ category: 'coding' as const, level: 5 }, { category: 'analysis' as const, level: 4 }] },
      { id: 'CodeReview', capabilities: [{ category: 'coding' as const, level: 4 }, { category: 'testing' as const, level: 3 }] },
      { id: 'Debug', capabilities: [{ category: 'coding' as const, level: 5 }, { category: 'testing' as const, level: 4 }] },
    ];
    const coverage = computeAgentTeamCapabilities(
      team,
      agents as unknown as Parameters<typeof computeAgentTeamCapabilities>[1],
    );
    expect(coverage.coding).toBeGreaterThan(0);
    expect(coverage.analysis).toBeGreaterThan(0);
    expect(coverage.testing).toBeGreaterThan(0);
  });
});

/**
 * Pure model for NodePalette — kept separate from the component file so the
 * component only exports React components (react-refresh contract).
 *
 * Source of truth (owner red-line): the enumerable agent types are driven by
 * the current user's config via `useAgentsList().allAgents`. It is NEVER a
 * hardcoded list — every registered agent type (Mode + SubAgent + user/project
 * custom + acp__* ACP bridge) appears, with hidden agents excluded.
 */
import type { AgentWithCapabilities } from '../agentsStore';
import { HIDDEN_AGENT_IDS } from '../agentVisibility';

export type NodePaletteCategory = 'mainClaw' | 'mode' | 'subagent' | 'acp' | 'custom';

export interface NodePaletteOption {
  id: string;
  name: string;
  description?: string;
  category: NodePaletteCategory;
}

/** Section render order for the enumerable agent types (leader rendered first). */
export const NODE_PALETTE_SECTION_ORDER: NodePaletteCategory[] = ['mode', 'subagent', 'acp', 'custom'];

/**
 * Enumerate every registered agent type, excluding hidden IDs.
 * Categories: ACP bridge (acp__*) > user/project custom > subagent > mode.
 */
export function buildNodePaletteOptions(
  agents: ReadonlyArray<AgentWithCapabilities>,
  hiddenAgentIds: ReadonlySet<string> = HIDDEN_AGENT_IDS,
): NodePaletteOption[] {
  const options: NodePaletteOption[] = [];
  for (const agent of agents) {
    if (hiddenAgentIds.has(agent.id)) continue;
    const source = agent.source ?? agent.subagentSource ?? 'builtin';
    let category: NodePaletteCategory;
    if (agent.id.startsWith('acp__')) {
      category = 'acp';
    } else if (source === 'user' || source === 'project') {
      category = 'custom';
    } else {
      category = agent.agentKind === 'subagent' ? 'subagent' : 'mode';
    }
    options.push({ id: agent.id, name: agent.name, description: agent.description, category });
  }
  return options;
}

/** Resolve the leader (main Claw = current user) option from the caller-provided id. */
export function resolveMainClawOption(
  mainClawAgentId: string | undefined,
  agents: ReadonlyArray<AgentWithCapabilities>,
): NodePaletteOption {
  const id = mainClawAgentId?.trim() || 'Claw';
  const agent = agents.find((candidate) => candidate.id === id);
  return {
    id,
    name: agent?.name ?? id,
    description: agent?.description,
    category: 'mainClaw',
  };
}

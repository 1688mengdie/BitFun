/**
 * NodePalette — selectable node-type palette for a legion/group workflow.
 *
 * Source of truth (owner red-line): the enumerable agent types are driven by
 * the current user's config via `useAgentsList().allAgents`. It is NEVER a
 * hardcoded list — every registered agent type (Mode + SubAgent + user/project
 * custom + acp__* ACP bridge) appears, with hidden agents excluded.
 *
 * The default/leader node is the main Claw = current user (creator): a type-level
 * Claw slot that is instantiated at create/save time. It is surfaced as the
 * first (default-highlighted) entry.
 */
import React, { useMemo } from 'react';
import type { TFunction } from 'i18next';
import type { AgentWithCapabilities } from '../agentsStore';
import { HIDDEN_AGENT_IDS } from '../agentVisibility';
import './NodePalette.scss';

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

export interface NodePaletteProps {
  /** `useAgentsList().allAgents` — full registered agent list (config-driven). */
  agents: AgentWithCapabilities[];
  hiddenAgentIds?: ReadonlySet<string>;
  /** Currently selected node type. Defaults to the main Claw when left blank. */
  selectedAgentId?: string;
  /** Main Claw = current user (creator). When omitted, the leader slot uses the Claw type. */
  mainClawAgentId?: string;
  onSelect?: (agentId: string) => void;
  t: TFunction<'scenes/agents'>;
}

const NodePalette: React.FC<NodePaletteProps> = ({
  agents,
  hiddenAgentIds = HIDDEN_AGENT_IDS,
  selectedAgentId,
  mainClawAgentId,
  onSelect,
  t,
}) => {
  const mainClaw = useMemo(
    () => resolveMainClawOption(mainClawAgentId, agents),
    [mainClawAgentId, agents],
  );
  const options = useMemo(
    () => buildNodePaletteOptions(agents, hiddenAgentIds),
    [agents, hiddenAgentIds],
  );

  const activeSelection = selectedAgentId ?? mainClaw.id;

  const sections = NODE_PALETTE_SECTION_ORDER
    .map((category) => ({
      category,
      label: t(`nodePalette.${category}`),
      items: options.filter((option) => option.category === category),
    }))
    .filter((section) => section.items.length > 0);

  const renderChip = (option: NodePaletteOption): React.ReactNode => {
    const isActive = option.id === activeSelection;
    return (
      <button
        key={option.id}
        type="button"
        className={`node-palette__option${isActive ? ' node-palette__option--active' : ''}`}
        data-agent-id={option.id}
        data-category={option.category}
        data-active={isActive}
        aria-pressed={isActive}
        onClick={() => onSelect?.(option.id)}
      >
        <span className="node-palette__option-name">{option.name}</span>
        {option.description ? (
          <span className="node-palette__option-desc" title={option.description}>
            {option.description}
          </span>
        ) : null}
        {option.category === 'mainClaw' ? (
          <span className="node-palette__option-badge">{t('nodePalette.mainClaw')}</span>
        ) : null}
      </button>
    );
  };

  return (
    <div className="node-palette" data-testid="node-palette" role="list" aria-label={t('nodePalette.title')}>
      <h3 className="node-palette__title">{t('nodePalette.title')}</h3>

      <div className="node-palette__section" data-testid="node-palette-main-claw">
        <h4 className="node-palette__section-title">{t('nodePalette.mainClaw')}</h4>
        <div className="node-palette__options">{renderChip(mainClaw)}</div>
      </div>

      {sections.map((section) => (
        <div key={section.category} className="node-palette__section" data-category={section.category}>
          <h4 className="node-palette__section-title">{section.label}</h4>
          <div className="node-palette__options">
            {section.items.map((option) => renderChip(option))}
          </div>
        </div>
      ))}
    </div>
  );
};

export default NodePalette;

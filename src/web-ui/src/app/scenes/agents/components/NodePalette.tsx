/**
 * NodePalette — selectable node-type palette for a legion/group workflow.
 *
 * The pure model (option enumeration / leader resolution) lives in
 * `NodePalette.model.ts` so this file only exports React components.
 */
import React, { useMemo } from 'react';
import type { TFunction } from 'i18next';
import type { AgentWithCapabilities } from '../agentsStore';
import { HIDDEN_AGENT_IDS } from '../agentVisibility';
import {
  buildNodePaletteOptions,
  NODE_PALETTE_SECTION_ORDER,
  resolveMainClawOption,
  type NodePaletteOption,
} from './NodePalette.model';
import './NodePalette.scss';

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
        data-bf-component="node-palette"
        data-bf-part="option"
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
    <div
      className="node-palette"
      data-testid="node-palette"
      role="list"
      aria-label={t('nodePalette.title')}
      data-bf-component="node-palette"
      data-bf-part="root"
    >
      <h3 className="node-palette__title" data-bf-component="node-palette" data-bf-part="title">{t('nodePalette.title')}</h3>

      <div
        className="node-palette__section"
        data-testid="node-palette-main-claw"
        data-bf-component="node-palette"
        data-bf-part="section"
      >
        <h4 className="node-palette__section-title">{t('nodePalette.mainClaw')}</h4>
        <div className="node-palette__options">{renderChip(mainClaw)}</div>
      </div>

      {sections.map((section) => (
        <div
          key={section.category}
          className="node-palette__section"
          data-category={section.category}
          data-bf-component="node-palette"
          data-bf-part="section"
        >
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

// @vitest-environment jsdom

import React, { act } from 'react';
import type { TFunction } from 'i18next';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import NodePalette from './NodePalette';
import {
  buildNodePaletteOptions,
  resolveMainClawOption,
} from './NodePalette.model';
import type { AgentWithCapabilities } from '../agentsStore';

let container: HTMLDivElement;
let root: Root | null = null;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) {
    await act(async () => { root!.unmount(); });
    root = null;
  }
  container.remove();
});

/** Build a minimal AgentWithCapabilities for palette tests. */
function makeAgent(
  partial: Partial<AgentWithCapabilities> & { id: string; name: string },
): AgentWithCapabilities {
  return {
    key: `key::${partial.id}`,
    id: partial.id,
    name: partial.name,
    description: partial.description ?? '',
    isReadonly: false,
    isReview: false,
    toolCount: 0,
    defaultTools: [],
    defaultEnabled: true,
    effectiveEnabled: true,
    capabilities: [],
    ...partial,
  } as AgentWithCapabilities;
}

const t = ((key: string, opts?: { defaultValue?: string }) => (
  opts?.defaultValue ?? key
)) as unknown as TFunction<'scenes/agents'>;

function render(element: React.ReactElement): void {
  act(() => { root!.render(element); });
}

describe('buildNodePaletteOptions', () => {
  it('excludes hidden agents and categorizes the rest (mode/subagent/custom/acp)', () => {
    const agents = [
      makeAgent({ id: 'agentic', name: 'Agentic', agentKind: 'mode' }),
      makeAgent({ id: 'CodeReview', name: 'CodeReview', agentKind: 'subagent' }),
      makeAgent({ id: 'my-custom', name: 'Custom', agentKind: 'subagent', source: 'user' }),
      makeAgent({ id: 'acp__codebuddy', name: 'CodeBuddy', agentKind: 'subagent', source: 'external' }),
      makeAgent({ id: 'Claw', name: 'Claw', agentKind: 'mode' }),
    ];
    const hidden = new Set(['Claw', 'DeepReview']);
    const options = buildNodePaletteOptions(agents, hidden);

    expect(options.find((o) => o.id === 'Claw')).toBeUndefined();
    expect(options.find((o) => o.id === 'agentic')?.category).toBe('mode');
    expect(options.find((o) => o.id === 'CodeReview')?.category).toBe('subagent');
    expect(options.find((o) => o.id === 'my-custom')?.category).toBe('custom');
    expect(options.find((o) => o.id === 'acp__codebuddy')?.category).toBe('acp');
  });
});

describe('resolveMainClawOption', () => {
  it('resolves the leader to the provided 主 Claw id', () => {
    const agents = [makeAgent({ id: 'Claw', name: 'Claw' })];
    const option = resolveMainClawOption('Claw', agents);
    expect(option.id).toBe('Claw');
    expect(option.category).toBe('mainClaw');
  });

  it('falls back to the Claw type when no id is provided', () => {
    const option = resolveMainClawOption(undefined, []);
    expect(option.id).toBe('Claw');
    expect(option.category).toBe('mainClaw');
  });
});

describe('NodePalette component', () => {
  it('renders the 主 Claw leader highlighted by default plus all non-hidden agent types', () => {
    const agents = [
      makeAgent({ id: 'agentic', name: 'Agentic', agentKind: 'mode' }),
      makeAgent({ id: 'my-custom', name: 'Custom', agentKind: 'subagent', source: 'user' }),
    ];
    const hidden = new Set(['Claw']);
    render(<NodePalette agents={agents} hiddenAgentIds={hidden} mainClawAgentId="Claw" t={t} />);

    const mainClaw = container.querySelector('[data-agent-id="Claw"]');
    expect(mainClaw).not.toBeNull();
    // 主 Claw is the default-highlighted node
    expect(mainClaw?.getAttribute('data-active')).toBe('true');

    // full enumeration of all (non-hidden) registered agent types
    expect(container.querySelector('[data-agent-id="agentic"]')).not.toBeNull();
    expect(container.querySelector('[data-agent-id="my-custom"]')).not.toBeNull();

    // hidden agents are not listed
    expect(container.querySelector('[data-agent-id="DeepReview"]')).toBeNull();
  });

  it('selects a different agent when onSelect is invoked', () => {
    const agents = [makeAgent({ id: 'agentic', name: 'Agentic', agentKind: 'mode' })];
    render(<NodePalette agents={agents} mainClawAgentId="agentic" t={t} />);

    const chip = container.querySelector('[data-agent-id="agentic"]') as HTMLElement | null;
    expect(chip?.getAttribute('data-active')).toBe('true');
  });
});

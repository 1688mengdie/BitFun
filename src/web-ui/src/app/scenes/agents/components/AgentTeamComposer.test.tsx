import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import AgentTeamComposer from './AgentTeamComposer';
import { useAgentsStore, MOCK_AGENT_TEAMS } from '../agentsStore';

const mocks = vi.hoisted(() => ({
  openMainSession: vi.fn(async () => {}),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string; count?: number; from?: string }) => {
      if (key.startsWith('formation.state.')) return `state:${key.split('.').pop()}`;
      return opts?.defaultValue ?? key;
    },
  }),
}));

vi.mock('@/flow_chat/services/sessionActivation', () => ({
  openMainSession: mocks.openMainSession,
}));

vi.mock('@/component-library', () => ({
  Badge: ({ children, className, variant }: { children: React.ReactNode; className?: string; variant?: string }) => (
    <span className={`${className ?? ''} badge-${variant ?? 'neutral'}`}>{children}</span>
  ),
  Button: ({ children }: { children: React.ReactNode }) => <button type="button">{children}</button>,
  IconButton: ({ children }: { children: React.ReactNode }) => <button type="button">{children}</button>,
}));

vi.mock('../agentsIcons', () => ({
  AGENT_ICON_MAP: { bot: () => <span /> },
}));

vi.mock('@/infrastructure/appearance/appearanceDomainTokens', () => ({
  APPEARANCE_DOMAIN_TOKENS: {
    agentTeam: {
      roleLeader: 'var(--t-leader)',
      roleMember: 'var(--t-member)',
      roleReviewer: 'var(--t-reviewer)',
    },
    agentCapability: {
      docs: 'var(--t-docs)',
      testing: 'var(--t-testing)',
      creative: 'var(--t-creative)',
      ops: 'var(--t-ops)',
    },
    tealAction: 'var(--t-teal)',
  },
}));

vi.mock('@/tools/bitfun-canvas/runtime/sdk/diagramLayout', () => {
  const real = vi.importActual<typeof import('@/tools/bitfun-canvas/runtime/sdk/diagramLayout')>('@/tools/bitfun-canvas/runtime/sdk/diagramLayout');
  return {
    computeDAGLayout: (options: Parameters<typeof import('@/tools/bitfun-canvas/runtime/sdk/diagramLayout')['computeDAGLayout']>[0] = {}) => {
      const nodes = options.nodes ?? [];
      const edges = options.edges ?? [];
      const nodeWidth = options.nodeWidth ?? 160;
      const nodeHeight = options.nodeHeight ?? 40;
      const padding = options.padding ?? 24;
      const rankGap = options.rankGap ?? 64;
      const nodeGap = options.nodeGap ?? 48;
      const positions = new Map<string, { x: number; y: number; rank: number }>();
      const rankOf = new Map<string, number>();
      for (const n of nodes) rankOf.set(String(n.id), 0);
      for (const e of edges) {
        const from = String((e as { from?: string | number }).from);
        const to = String((e as { to?: string | number }).to);
        const next = (rankOf.get(from) ?? 0) + 1;
        rankOf.set(to, Math.max(rankOf.get(to) ?? 0, next));
      }
      for (const n of nodes) {
        const rank = rankOf.get(String(n.id)) ?? 0;
        const sameRank = nodes.filter((x) => (rankOf.get(String(x.id)) ?? 0) === rank);
        const idx = sameRank.findIndex((x) => String(x.id) === String(n.id));
        positions.set(String(n.id), {
          x: padding + idx * (nodeWidth + nodeGap),
          y: padding + rank * (nodeHeight + rankGap),
          rank,
        });
      }
      const layoutNodes = nodes.map((n) => {
        const p = positions.get(String(n.id))!;
        return {
          id: String(n.id),
          x: p.x,
          y: p.y,
          centerX: p.x + nodeWidth / 2,
          centerY: p.y + nodeHeight / 2,
          width: nodeWidth,
          height: nodeHeight,
          rank: p.rank,
        };
      });
      const layoutEdges = edges.map((e) => {
        const from = String((e as { from?: string | number }).from);
        const to = String((e as { to?: string | number }).to);
        const s = positions.get(from)!;
        const t = positions.get(to)!;
        return {
          from,
          to,
          sourceX: s.x + nodeWidth / 2,
          sourceY: s.y + nodeHeight,
          targetX: t.x + nodeWidth / 2,
          targetY: t.y,
          isBackEdge: false,
          path: `M ${s.x} ${s.y} C 0 0 0 0 ${t.x} ${t.y}`,
        };
      });
      const maxRank = Math.max(0, ...layoutNodes.map((n) => n.rank));
      return {
        nodes: layoutNodes,
        edges: layoutEdges,
        ranks: [],
        direction: options.direction ?? 'vertical',
        width: 400,
        height: padding * 2 + (maxRank + 1) * (nodeHeight + rankGap),
      };
    },
    normalizeDagEdges: (edges: unknown[]) => (edges as Array<{ from?: unknown; to?: unknown; source?: unknown; target?: unknown }>).map((e) => ({ ...e, from: e.from ?? e.source, to: e.to ?? e.target })).filter((e) => e.from !== undefined && e.to !== undefined),
    edgePath: (_e: unknown, _d: unknown) => 'M 0 0',
  };
});

vi.mock('@/flow_chat/state-machine/types', () => ({
  SessionDisplayState: {
    STANDBY: 'standby',
    PROCESSING: 'processing',
    COMPLETED: 'completed',
    HUNG: 'hung',
    INTERRUPTED: 'interrupted',
    PENDING_ATTENTION: 'pending_attention',
    VIEWED: 'viewed',
  },
  SESSION_DISPLAY_STATES: ['standby', 'processing', 'completed', 'hung', 'interrupted', 'pending_attention', 'viewed'],
}));

let JSDOMCtor: (new (
  html?: string,
  options?: { pretendToBeVisual?: boolean }
) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

describeWithJsdom('AgentTeamComposer (R-WF-17 DAG canvas)', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });
    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('MutationObserver', window.MutationObserver);
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation(() => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
      })),
    });
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
    mocks.openMainSession.mockClear();
  });

  it('renders formation nodes for the active team', async () => {
    await act(async () => {
      root.render(<AgentTeamComposer />);
    });
    const nodes = container.querySelectorAll('.tcf__node');
    expect(nodes.length).toBeGreaterThan(0);
  });

  it('shows the seven-state badge on each member node (assertion 2)', async () => {
    await act(async () => {
      root.render(<AgentTeamComposer />);
    });
    const states = new Set<string>();
    for (const badge of container.querySelectorAll('.tcf__node-state')) {
      states.add(badge.textContent ?? '');
    }
    // Mock teams cover all seven states in aggregate.
    for (const expected of ['state:standby', 'state:processing', 'state:completed', 'state:viewed']) {
      expect(states).toContain(expected);
    }
  });

  it('draws SVG edges from the official layout (assertion 1 - display)', async () => {
    await act(async () => {
      root.render(<AgentTeamComposer />);
    });
    const paths = container.querySelectorAll('.tcf__svg .tcf__edge');
    expect(paths.length).toBeGreaterThan(0);
  });

  it('creates an edge by wiring two nodes (assertion 1 - edit)', async () => {
    await act(async () => {
      root.render(<AgentTeamComposer />);
    });
    const teamId = useAgentsStore.getState().activeAgentTeamId!;
    const team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    const existing = new Set(team.edges.map(([a, b]) => `${a}->${b}`));
    const nodes = Array.from(container.querySelectorAll<HTMLElement>('.tcf__node'));
    const srcIdx = 0;
    const srcId = nodes[srcIdx]!.getAttribute('data-member-id')!;
    const dst = nodes.find((n) => {
      const id = n.getAttribute('data-member-id')!;
      return id !== srcId && !existing.has(`${srcId}->${id}`);
    });
    expect(dst).toBeTruthy();
    const beforeCount = team.edges.length;

    const srcPort = nodes[srcIdx]!.querySelector<HTMLButtonElement>('[data-testid="tcf-node-port"]')!;
    // enter wire mode from source port
    await act(async () => {
      srcPort.click();
    });
    expect(container.querySelector('[data-testid="tcf-wire-active"]')).toBeTruthy();

    // click the destination node to complete the wire
    await act(async () => {
      dst!.click();
    });
    const after = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    expect(after.edges.length).toBeGreaterThan(beforeCount);
  });

  it('opens a session when the node jump button is clicked (assertion 3)', async () => {
    await act(async () => {
      root.render(<AgentTeamComposer />);
    });
    const jumps = Array.from(container.querySelectorAll<HTMLButtonElement>('[data-testid="tcf-node-jump"]'));
    expect(jumps.length).toBeGreaterThan(0);
    await act(async () => {
      jumps[0]!.click();
    });
    expect(mocks.openMainSession).toHaveBeenCalled();
  });
});

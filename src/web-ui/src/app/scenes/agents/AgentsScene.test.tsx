// @vitest-environment jsdom

import React, { act } from 'react';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import React from 'react';
import { useAgentsStore } from './agentsStore';
import { isLocallyManageableSubagent } from './agentVisibility';

const useAgentsListMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}));

vi.mock('./components/CreateAgentPage', () => ({
  default: () => <div data-testid="create-agent-page">create agent</div>,
}));

vi.mock('./components/AgentCard', () => ({
  default: ({
    agent,
    toolCount,
    onOpenDetails,
  }: {
    agent: { name: string };
    toolCount?: number;
    onOpenDetails: (agent: unknown) => void;
  }) => (
    <button
      type="button"
      data-tool-count={toolCount}
      onClick={() => onOpenDetails(agent)}
    >
      {agent.name}
    </button>
  ),
}));

vi.mock('./components/CoreAgentCard', () => ({
  default: () => <div />,
}));

vi.mock('./components/useUserToolGroups', () => ({
  useUserToolGroups: () => ({
    groups: [],
    loading: false,
    saveGroups: vi.fn(),
  }),
}));

vi.mock('./components/useUserSkillGroups', () => ({
  useUserSkillGroups: () => ({
    groups: [],
    loading: false,
    saveGroups: vi.fn(),
  }),
}));

vi.mock('./components/SkillGroupPicker', () => ({
  SkillGroupPicker: () => <div data-testid="agent-detail-skill-groups">skill picker</div>,
  SkillGroupSummary: () => <div data-testid="agent-detail-skill-summary">skill summary</div>,
}));

vi.mock('./components/ToolGroupPicker', () => ({
  ToolGroupPicker: ({ tools }: { tools: Array<{ name: string }> }) => (
    <div data-testid="agent-detail-tool-groups">
      {tools.map((tool) => tool.name).join(',')}
    </div>
  ),
  ToolGroupSummary: ({ tools }: { tools: Array<{ name: string }> }) => (
    <div data-testid="agent-detail-tool-summary">
      {tools.map((tool) => tool.name).join(',')}
    </div>
  ),
}));

vi.mock('@/component-library', () => ({
  Badge: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Button: ({ children, onClick, disabled, variant, 'data-testid': testId }: {
    children: React.ReactNode;
    onClick?: () => void;
    disabled?: boolean;
    variant?: string;
    'data-testid'?: string;
  }) => (
    <button type="button" onClick={onClick} disabled={disabled} data-testid={testId} data-bf-variant={variant}>{children}</button>
  ),
  IconButton: ({ children, onClick, 'data-testid': testId, 'aria-label': ariaLabel }: { children: React.ReactNode; onClick?: () => void; 'data-testid'?: string; 'aria-label'?: string }) => (
    <button type="button" onClick={onClick} data-testid={testId} aria-label={ariaLabel}>{children}</button>
  ),
  Search: () => <input readOnly />,
  Select: () => <div />,
  Switch: () => <input type="checkbox" readOnly />,
  confirmDanger: vi.fn(async () => false),
}));

vi.mock('@/app/components', () => ({
  GalleryDetailModal: ({ children, actions }: { children?: React.ReactNode; actions?: React.ReactNode }) => (
    <div>{children}{actions}</div>
  ),
  GalleryEmpty: () => <div />,
  GalleryGrid: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  GalleryLayout: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <main className={className}>{children}</main>
  ),
  GalleryPageHeader: ({ extraContent, actions }: { extraContent?: React.ReactNode; actions?: React.ReactNode }) => (
    <header>{extraContent}{actions}</header>
  ),
  GallerySkeleton: () => <div />,
  // Spread props so data-testid/id reach the DOM like the real GalleryZone
  // (production spreads ...sectionProps onto <section>).
  GalleryZone: ({ children, tools, ...props }: { children: React.ReactNode; tools?: React.ReactNode } & React.HTMLAttributes<HTMLElement>) => (
    <section {...props}>{tools}{children}</section>
  ),
}));

vi.mock('./hooks/useAgentsList', () => ({
  useAgentsList: () => useAgentsListMock(),
}));

function mockAgentsList(overrides: Record<string, unknown> = {}) {
  useAgentsListMock.mockReturnValue({
    allAgents: [],
    filteredAgents: [],
    loading: false,
    availableTools: [],
    getModeProfile: () => null,
    getAgentSkills: () => [],
    getModeManageableSubagents: () => [],
    counts: { builtin: 0, user: 0, project: 0, mode: 0, subagent: 0 },
    loadAgents: vi.fn(),
    getModeConfig: () => undefined,
    handleSetTools: vi.fn(),
    handleResetTools: vi.fn(),
    handleSetSkills: vi.fn(),
    handleResetSkills: vi.fn(),
    handleSetSubagentEnabled: vi.fn(),
    handleSetSubagentModel: vi.fn(),
    ...overrides,
  });
}

vi.mock('@/app/hooks/useGallerySceneAutoRefresh', () => ({
  useGallerySceneAutoRefresh: vi.fn(),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({ workspacePath: 'D:/workspace/project' }),
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {
    getConfig: vi.fn(async () => false),
    onConfigChange: vi.fn(() => () => {}),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/api/service-api/SubagentAPI', () => ({
  SubagentAPI: {
    deleteSubagent: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api/service-api/LegionPresetAPI', () => ({
  LegionPresetAPI: {
    createPreset: vi.fn(async () => {}),
    listPresets: vi.fn(async () => []),
  },
}));

vi.mock('./components/LegionCard', () => ({
  default: ({ pattern }: { pattern: { id: string; name: string } }) => (
    <div data-testid="legion-list-item" data-legion-id={pattern.id}>{pattern.name}</div>
  ),
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

describeWithJsdom('AgentsScene', () => {

  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    // The jsdom environment (via the `// @vitest-environment jsdom` pragma)
    // provides a real document before react-dom initializes its event system,
    // so controlled input events dispatch like a real browser.
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.stubGlobal('MutationObserver', window.MutationObserver);
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
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

    useAgentsStore.getState().openHome();
    mockAgentsList();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
    useAgentsStore.getState().openHome();
  });

  // --- Batch B: AgentsScene zone/action layout (P1-1/P1-2/P1-3/P1-4/P1-7) ---

  it('orders agents-zone tools with the primary create-agent action first', async () => {
    const { default: AgentsScene } = await import('./AgentsScene');
    await act(async () => {
      root.render(<AgentsScene />);
    });

    const zone = container.querySelector('[data-testid="agents-custom-zone"]');
    expect(zone).toBeTruthy();
    const toolIds = Array.from(zone?.querySelectorAll<HTMLElement>('[data-testid]') ?? [])
      .map((el) => el.getAttribute('data-testid'));
    const createIdx = toolIds.indexOf('agents-create-agent-btn');
    const legionIdx = toolIds.indexOf('agents-create-legion-btn');
    const reviewIdx = toolIds.indexOf('agents-open-review-team-btn');
    expect(createIdx).toBeGreaterThanOrEqual(0);
    expect(legionIdx).toBeGreaterThan(createIdx);
    expect(reviewIdx).toBeGreaterThan(legionIdx);
    // The create-agent button carries the primary highlight.
    const createBtn = zone?.querySelector('[data-testid="agents-create-agent-btn"]');
    expect(createBtn?.className).toContain('gallery-action-btn--primary');
    // A visual separator sits between the primary action and secondary ones.
    const seps = Array.from(zone?.querySelectorAll('.gallery-action-sep') ?? []);
    expect(seps.length).toBeGreaterThanOrEqual(1);
  });

  it('keeps top-level zones flat and adds all four anchors', async () => {
    const { default: AgentsScene } = await import('./AgentsScene');
    const { LegionPresetAPI } = await import('@/infrastructure/api/service-api/LegionPresetAPI');
    const listPresets = LegionPresetAPI.listPresets as ReturnType<typeof vi.fn>;
    listPresets.mockResolvedValue([
      {
        id: 'sparc-dev',
        name: 'SPARC Development',
        description: '5-stage pipeline',
        nodes: [],
        edges: [],
      },
    ]);

    await act(async () => {
      root.render(<AgentsScene />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const zones = Array.from(container.querySelectorAll<HTMLElement>('section[id]'))
      .map((s) => s.getAttribute('id'));
    expect(zones).toContain('core-agents-zone');
    expect(zones).toContain('agents-zone');
    expect(zones).toContain('legions-zone');
    expect(zones).toContain('agent-teams-zone');

    // The teams zone is no longer nested inside agents-zone.
    const agentsZone = container.querySelector('[data-testid="agents-custom-zone"]');
    const teamsZone = container.querySelector('[data-testid="agents-teams-zone"]');
    expect(agentsZone?.contains(teamsZone ?? null)).toBe(false);

    // Anchor bar exposes all four zones.
    for (const testId of [
      'agents-anchor-core',
      'agents-anchor-custom',
      'agents-anchor-legions',
      'agents-anchor-teams',
    ]) {
      expect(container.querySelector(`[data-testid="${testId}"]`)).toBeTruthy();
    }
  });

  it('marks the delete button as danger and keeps it separated from edit', async () => {
    const subagent = {
      key: 'user::delete-me',
      id: 'delete-me',
      name: 'Delete me',
      description: 'Custom subagent.',
      isReadonly: false,
      isReview: false,
      toolCount: 0,
      defaultTools: [],
      defaultEnabled: true,
      effectiveEnabled: true,
      source: 'user',
      agentKind: 'subagent' as const,
      capabilities: [],
    };
    mockAgentsList({
      allAgents: [subagent],
      filteredAgents: [subagent],
    });
    const { default: AgentsScene } = await import('./AgentsScene');

    await act(async () => {
      root.render(<AgentsScene />);
    });
    await act(async () => {
      Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
        .find((button) => button.textContent === subagent.name)
        ?.click();
    });

    const deleteBtn = Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent === 'agentsOverview.deleteAgent');
    expect(deleteBtn).toBeTruthy();
    expect(deleteBtn?.getAttribute('data-bf-variant')).toBe('danger');
    const actionsRow = deleteBtn?.parentElement;
    expect(actionsRow?.getAttribute('style')).toMatch(/gap:\s*16/);
    const editBtn = Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent === 'agentsOverview.editAgent');
    expect(editBtn).toBeTruthy();
  });

  it('opens the team editor from the details modal with the save-chained action', async () => {
    const { default: AgentsScene } = await import('./AgentsScene');
    await act(async () => {
      root.render(<AgentsScene />);
    });

    const teamName = useAgentsStore.getState().agentTeams[0]?.name ?? '';
    const card = Array.from(container.querySelectorAll<HTMLElement>('.agent-team-card'))
      .find((el) => el.getAttribute('aria-label') === teamName);
    expect(card).toBeTruthy();
    await act(async () => {
      card?.click();
    });

    const editAction = Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent === 'composer.saveTeam');
    expect(editAction).toBeTruthy();
    await act(async () => {
      editAction?.click();
    });
    expect(container.querySelector('.bitfun-agents-scene--page')).toBeTruthy();
  });

  it('surfaces an unsupported tool catalog in the tools tab instead of an empty list', async () => {
    // When the host can't answer get_all_tools_info the tools tab must say so
    // and disable editing, rather than rendering as "no tools". See PR #2428
    // round 5 #2.
    const mode = {
      key: 'mode::custom-mode',
      id: 'custom-mode',
      name: 'Custom mode',
      description: 'General coding mode.',
      isReadonly: false,
      isReview: false,
      toolCount: 1,
      defaultTools: ['Read'],
      defaultEnabled: true,
      effectiveEnabled: true,
      source: 'user',
      agentKind: 'mode' as const,
      capabilities: [],
    };
    mockAgentsList({
      allAgents: [mode],
      filteredAgents: [mode],
      availableTools: [],
      toolCatalogStatus: 'unsupported',
      getModeConfig: () => ({
        profile_id: 'custom-mode',
        enabled_tools: ['Read'],
        default_tools: ['Read'],
      }),
    });
    const { default: AgentsScene } = await import('./AgentsScene');

    await act(async () => {
      root.render(<AgentsScene />);
    });
    await act(async () => {
      Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
        .find((button) => button.textContent === mode.name)
        ?.click();
    });

    const status = container.querySelector('[data-testid="agent-detail-tools-catalog-status"]');
    expect(status?.textContent).toContain('agentsOverview.toolsUnsupported');
    // The tool summary picker must not render — the catalog is not available.
    expect(container.querySelector('[data-testid="agent-detail-tool-summary"]')).toBeNull();
  });
});

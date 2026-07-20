// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequest } from '@/infrastructure/api/service-api/AgentAPI';
import { PermissionRequestPanel } from './PermissionRequestPanel';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, string>) => {
      if (key === 'permission.subagentOwner') {
        return `${values?.subagent} subagent`;
      }
      if (key === 'permission.allowAlwaysTooltip') {
        return `Always allow saves matching access for ${values?.projectPath}`;
      }
      if (key === 'permission.actions.edit') {
        return 'Edit files';
      }
      if (key === 'permission.actions.bash') {
        return 'Run command';
      }
      return key;
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ content, children }: { content: string; children: React.ReactElement }) => (
    <span data-tooltip={content}>{children}</span>
  ),
}));

vi.mock('../../store/chatInputStateStore', () => ({
  useChatInputState: () => 0,
}));

function request(delegated: boolean): PermissionRequest {
  return {
    requestId: delegated ? 'child-request' : 'direct-request',
    roundId: delegated ? 'round-child' : 'round-parent',
    order: 0,
    sessionId: delegated ? 'child-session' : 'parent-session',
    toolCallId: delegated ? 'child-tool' : 'direct-tool',
    projectPath: '/workspace/BitFun',
    projectId: 'project-1',
    agentId: delegated ? 'Explore' : 'agentic',
    action: 'edit',
    resources: ['src/main.rs'],
    saveResources: ['src/main.rs'],
    source: { kind: 'tool_call', identity: 'Write' },
    delegation: delegated
      ? {
          parentSessionId: 'parent-session',
          parentDialogTurnId: 'parent-turn',
          parentToolCallId: 'parent-task',
          subagentType: 'Explore',
        }
      : undefined,
  };
}

describe('PermissionRequestPanel', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('names the subagent that owns a delegated permission request', () => {
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[request(true)]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('Explore subagent');
    expect(container.querySelector('.permission-request-panel__heading h2')?.textContent)
      .toBe('permission.title');
  });

  it('keeps direct request details in the request row and scopes always allow to the project path', () => {
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[request(false)]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('Edit files');
    expect(container.textContent).toContain('Write');
    expect(container.textContent).not.toContain('edit');
    expect(container.textContent).not.toContain('subagent');
    const tooltips = [...container.querySelectorAll('[data-tooltip]')]
      .map((node) => node.getAttribute('data-tooltip'));
    expect(tooltips).toContain('Always allow saves matching access for /workspace/BitFun');
    expect(tooltips).not.toContain('project-1');
  });

  it('keeps resources to one ellipsized summary with the complete value in a tooltip', () => {
    const longResource = 'src/a-very-long-directory-name/another-long-directory/file-with-a-long-name.ts';
    const bashRequest = {
      ...request(false),
      action: 'bash',
      resources: [longResource, 'pnpm run type-check:web'],
    };
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[bashRequest]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    const resourceSummary = container.querySelector('.permission-request-panel__resource-summary');
    expect(resourceSummary?.textContent).toBe(`${longResource}, pnpm run type-check:web`);
    expect(resourceSummary?.parentElement?.getAttribute('data-tooltip'))
      .toBe(`${longResource}, pnpm run type-check:web`);
    expect(container.textContent).toContain('Run command');
  });

  it('shows one ordered batch and responds to the current and following requests once', async () => {
    const first = request(false);
    const second = { ...request(false), requestId: 'second-request', order: 1 };
    const onRespondBatch = vi.fn(() => Promise.resolve());
    await act(async () => {
      root.render(
        <PermissionRequestPanel
          requests={[first, second]}
          onRespond={vi.fn()}
          onRespondBatch={onRespondBatch}
        />,
      );
    });

    const batchButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('permission.allowCurrentAndFollowing'),
    );
    expect(batchButton).toBeDefined();
    await act(async () => {
      batchButton?.click();
      await Promise.resolve();
    });

    expect(onRespondBatch).toHaveBeenCalledWith(first.requestId, 'once', undefined);
    expect(container.querySelectorAll('[role="listitem"]')).toHaveLength(2);
  });
});

// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PermissionV2Request } from '@/infrastructure/api/service-api/AgentAPI';
import { PermissionRequestPanel } from './PermissionRequestPanel';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, string>) => {
      if (key === 'permissionV2.subagentRequest') {
        return `${values?.subagent} subagent · ${values?.action} · ${values?.tool}`;
      }
      return key;
    },
  }),
}));

vi.mock('../../store/chatInputStateStore', () => ({
  useChatInputState: () => 0,
}));

function request(delegated: boolean): PermissionV2Request {
  return {
    requestId: delegated ? 'child-request' : 'direct-request',
    roundId: delegated ? 'round-child' : 'round-parent',
    order: 0,
    sessionId: delegated ? 'child-session' : 'parent-session',
    toolCallId: delegated ? 'child-tool' : 'direct-tool',
    projectId: 'project-1',
    agentId: delegated ? 'Explore' : 'agentic',
    action: 'edit',
    resources: ['src/main.rs'],
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

    expect(container.textContent).toContain('Explore subagent · edit · Write');
  });

  it('preserves the direct request description', () => {
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[request(false)]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('edit · Write');
    expect(container.textContent).not.toContain('subagent');
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
      (button) => button.textContent?.includes('permissionV2.allowCurrentAndFollowing'),
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

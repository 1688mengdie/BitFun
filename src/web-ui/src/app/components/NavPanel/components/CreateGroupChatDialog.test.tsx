// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SessionMetadata } from '@/shared/types/session-history';

vi.mock('@/component-library', () => {
  const React = require('react');
  return {
    Modal: ({ isOpen, children }: { isOpen: boolean; children: React.ReactNode }) =>
      isOpen ? <div data-testid="modal">{children}</div> : null,
    Input: (props: { label?: string; value?: string; onChange?: (e: { target: { value: string } }) => void; placeholder?: string; autoFocus?: boolean }) => (
      <input
        data-testid="group-name-input"
        aria-label={props.label}
        placeholder={props.placeholder}
        value={props.value ?? ''}
        onChange={props.onChange}
        autoFocus={props.autoFocus}
      />
    ),
    Checkbox: (props: { checked?: boolean; onChange?: () => void; label?: string; size?: string; disabled?: boolean }) => (
      <input
        type="checkbox"
        data-testid={props.label ? 'member-select-all' : 'member-checkbox'}
        checked={props.checked}
        onChange={props.onChange}
        aria-label={props.label}
        disabled={props.disabled}
      />
    ),
    Button: (props: { onClick?: () => void; disabled?: boolean; isLoading?: boolean; variant?: string; children?: React.ReactNode; type?: string; size?: string }) => (
      <button
        type={props.type ?? 'button'}
        data-testid={props.variant === 'primary' ? 'group-create-submit' : 'group-cancel'}
        onClick={props.onClick}
        disabled={props.disabled}
      >
        {props.children}
      </button>
    ),
  };
});

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/infrastructure/api/service-api/ToolAPI', () => ({
  toolAPI: {
    executeTool: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: {
    listSessions: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
  },
}));

import CreateGroupChatDialog from './CreateGroupChatDialog';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';

const makeSession = (id: string, agentType: string, sessionName?: string): SessionMetadata => ({
  sessionId: id,
  sessionName: sessionName ?? id,
  agentType,
  modelName: 'auto',
  createdAt: 0,
  lastActiveAt: 0,
  turnCount: 0,
  messageCount: 0,
  toolCallCount: 0,
  status: 'active',
  tags: [],
});

describe('CreateGroupChatDialog (R-GC-13)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.mocked(toolAPI.executeTool).mockReset();
    vi.mocked(sessionAPI.listSessions).mockReset();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const renderDialog = (onCreated = vi.fn(), onClose = vi.fn()) => {
    act(() => {
      root.render(
        <CreateGroupChatDialog
          isOpen
          onClose={onClose}
          workspacePath="/workspace-a"
          onCreated={onCreated}
        />,
      );
    });
    return { onCreated, onClose };
  };

  const setGroupName = (value: string) => {
    const input = document.querySelector<HTMLInputElement>('[data-testid="group-name-input"]');
    expect(input).not.toBeNull();
    act(() => {
      // React 受控组件：必须用原生 value setter 绕过 React 的 value 锁定，再派发 input 事件。
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSetter?.call(input, value);
      input!.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };

  const clickCreate = () => {
    const button = document.querySelector<HTMLButtonElement>('[data-testid="group-create-submit"]');
    expect(button).not.toBeNull();
    act(() => button!.click());
  };

  const getSubmitDisabled = () => {
    const button = document.querySelector<HTMLButtonElement>('[data-testid="group-create-submit"]');
    return button?.disabled ?? true;
  };

  it('filters member picker to Claw sessions only', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('code-1', 'agentic', 'Code B'),
      makeSession('claw-2', 'Claw', 'Assist C'),
    ]);
    renderDialog();

    await act(async () => { await Promise.resolve(); });
    // 成员行渲染 label；代码只渲染 agentType === 'Claw' 的会话
    const rows = [...document.querySelectorAll('label.group-chat-dialog__member-row')];
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain('Assist A');
    expect(rows[1]!.textContent).toContain('Assist C');
  });

  it('unions assistant workspace presets as inactive Claw members (R-GC-19)', async () => {
    // 当前项目工作区没有 Claw 会话；assistant workspace 预设兜底（未激活标记）。
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('code-1', 'agentic', 'Code B'),
    ]);
    const assistantWorkspaces = [
      { id: 'local_aaa', name: '姬码锋', rootPath: '/ws/a', workspaceKind: 'assistant', assistantId: 'bd56fce3', workspaceType: 'other', languages: [], openedAt: '', lastAccessed: '', tags: [] },
      { id: 'local_bbb', name: '姬梦情', rootPath: '/ws/b', workspaceKind: 'assistant', workspaceType: 'other', languages: [], openedAt: '', lastAccessed: '', tags: [] },
    ];
    act(() => {
      root.render(
        <CreateGroupChatDialog
          isOpen
          onClose={() => {}}
          workspacePath="/workspace-a"
          assistantWorkspaces={assistantWorkspaces as any}
          onCreated={() => {}}
        />,
      );
    });
    await act(async () => { await Promise.resolve(); });

    const rows = [...document.querySelectorAll('label.group-chat-dialog__member-row')];
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain('姬码锋');
    expect(rows[0]!.querySelector('[data-bf-part="inactiveBadge"]')).not.toBeNull();
    expect(rows[1]!.textContent).toContain('姬梦情');
  });

  it('creates the group through toolAPI.executeTool with camelCase shape (no direct invoke)', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist C'),
    ]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-1' },
      error: null,
      validation_error: null,
      duration_ms: 1,
    });
    const { onCreated } = renderDialog();
    await act(async () => { await Promise.resolve(); });

    setGroupName(' 项目群 ');
    expect(getSubmitDisabled()).toBe(false);
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool).toHaveBeenCalledWith({
      toolName: 'create_group_chat',
      parameters: { action: 'create', name: '项目群', members: [], workspace: '/workspace-a' },
      workspacePath: '/workspace-a',
    });
    expect(onCreated).toHaveBeenCalledWith('group-1', '项目群');
  });

  it('omits workspace parameter when workspacePath is empty (backend default fallback, R-GC-17)', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
    ]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-empty-ws' },
    });
    act(() => {
      root.render(
        <CreateGroupChatDialog
          isOpen
          onClose={() => {}}
          workspacePath=""
          onCreated={() => {}}
        />,
      );
    });
    await act(async () => { await Promise.resolve(); });

    setGroupName('空工作区群');
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool).toHaveBeenCalledWith({
      toolName: 'create_group_chat',
      parameters: { action: 'create', name: '空工作区群', members: [], workspace: undefined },
      workspacePath: '',
    });
  });

  it('passes selected member ids and navigates on success', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist C'),
    ]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-9' },
    });
    const { onCreated } = renderDialog();
    await act(async () => { await Promise.resolve(); });

    // 勾选第一个成员（排除 header 的全选 Checkbox，data-testid 为 member-checkbox）
    const checkboxes = [...document.querySelectorAll<HTMLInputElement>('[data-testid="member-checkbox"]')];
    act(() => checkboxes[0]!.click());
    setGroupName('群A');
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledWith(expect.objectContaining({
      parameters: { action: 'create', name: '群A', members: ['claw-1'], workspace: '/workspace-a' },
    }));
    expect(onCreated).toHaveBeenCalledWith('group-9', '群A');
  });

  it('surfaces backend failure without calling onCreated', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: false,
      result: null,
      error: 'name is required for create',
      validation_error: null,
      duration_ms: 1,
    });
    const { onCreated } = renderDialog();
    await act(async () => { await Promise.resolve(); });

    setGroupName('空');
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(onCreated).not.toHaveBeenCalled();
  });
});

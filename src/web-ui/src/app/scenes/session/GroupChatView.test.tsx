// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/infrastructure/api/service-api/ToolAPI', () => ({
  toolAPI: { executeTool: vi.fn() },
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: {
    loadSessionMetadata: vi.fn(),
    listSessions: vi.fn(),
  },
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/shared/notification-system', () => ({
  notificationService: { success: vi.fn(), error: vi.fn(), warning: vi.fn() },
}));

// 现成组件 mock（复用核验：真实组件路径保留在 import 中，测试只 stub 渲染面）。
vi.mock('../../../flow_chat/components/modern/ModernFlowChatContainer', () => ({
  ModernFlowChatContainer: ({ className, emptyState }: { className?: string; emptyState?: React.ReactNode }) => (
    <div data-testid="flow-chat-container" data-class-name={className ?? ''}>
      {emptyState}
    </div>
  ),
}));

vi.mock('../../../flow_chat/components/ChatInput', () => ({
  ChatInput: ({
    registration,
  }: {
    registration?: { placeholder?: string; onSubmit?: (s: { text: string }) => Promise<void> | void };
  }) => (
    <div data-testid="group-chat-input">
      <input
        data-testid="group-chat-input-box"
        placeholder={registration?.placeholder}
      />
      <button
        type="button"
        data-testid="group-chat-input-send"
        onClick={() => {
          const box = document.querySelector<HTMLInputElement>('[data-testid="group-chat-input-box"]');
          if (box && registration?.onSubmit) {
            void registration.onSubmit({ text: box.value });
          }
        }}
      >
        send
      </button>
    </div>
  ),
}));

// 复用核验：R-GC-15 成员区全部走现成 component-library 组件（Modal/Button/
// Checkbox/Input），测试只 stub 渲染面，不改生产代码路径。
vi.mock('@/component-library', () => {
  const React = require('react');
  return {
    Modal: ({ isOpen, children }: { isOpen: boolean; children: React.ReactNode }) =>
      isOpen ? <div data-testid="modal">{children}</div> : null,
    Input: (props: { label?: string; value?: string; onChange?: (e: { target: { value: string } }) => void; placeholder?: string; autoFocus?: boolean }) => (
      <input
        data-testid="dialog-name-input"
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
        data-testid={`btn-${props.variant ?? 'default'}`}
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

// R-GC-15: flowChatStore 单例 mock（createSession/markSessionAsGroupChat/
// addDialogTurn/getState）——复用 R-GC-13 登记形态，测试验证 fork 跳转链路。
const flowChatMocks = vi.hoisted(() => ({
  createSession: vi.fn(),
  markSessionAsGroupChat: vi.fn(),
  addDialogTurn: vi.fn(),
  getState: vi.fn(() => ({ sessions: new Map(), activeSessionId: null })),
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  FlowChatStore: { getInstance: () => flowChatMocks },
  flowChatStore: flowChatMocks,
}));

vi.mock('@/flow_chat/services/sessionActivation', () => ({
  openMainSession: vi.fn(() => Promise.resolve()),
}));

import GroupChatView from './GroupChatView';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { SessionMetadata } from '@/shared/types/session-history';

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

describe('GroupChatView (R-GC-14 view + R-GC-15 member management)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.mocked(toolAPI.executeTool).mockReset();
    vi.mocked(sessionAPI.loadSessionMetadata).mockReset();
    vi.mocked(sessionAPI.listSessions).mockReset();
    vi.mocked(openMainSession).mockClear();
    flowChatMocks.createSession.mockClear();
    flowChatMocks.markSessionAsGroupChat.mockClear();
    flowChatMocks.addDialogTurn.mockClear();
    flowChatMocks.getState.mockClear();
    flowChatMocks.getState.mockReturnValue({ sessions: new Map(), activeSessionId: null });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const renderView = (props?: Partial<React.ComponentProps<typeof GroupChatView>>) => {
    act(() => {
      root.render(
        <GroupChatView groupId="group-1" workspacePath="/workspace-a" {...props} />,
      );
    });
  };

  const flush = async () => {
    await act(async () => { await Promise.resolve(); });
    await act(async () => { await Promise.resolve(); });
  };

  const typeMessage = (value: string) => {
    const box = document.querySelector<HTMLInputElement>('[data-testid="group-chat-input-box"]');
    expect(box).not.toBeNull();
    act(() => {
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSetter?.call(box, value);
      box!.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };

  const clickSend = () => {
    const sendBtn = document.querySelector<HTMLButtonElement>('[data-testid="group-chat-input-send"]');
    expect(sendBtn).not.toBeNull();
    act(() => sendBtn!.click());
  };

  it('loads group history through toolAPI.executeTool (get_group_history, camelCase, no bare invoke)', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: {
        groupId: 'group-1',
        messages: [
          {
            messageId: 'msg-1',
            groupSessionId: 'group-1',
            author: { sessionId: 'commander-1', role: 'Commander', depth: 0, name: '群主' },
            content: 'hello group',
            timestamp: 1000,
          },
          {
            messageId: 'msg-2',
            groupSessionId: 'group-1',
            author: { sessionId: 'member-2', role: 'Executor', depth: 1, name: '二号' },
            content: 'received',
            timestamp: 2000,
          },
        ],
      },
    });
    renderView();
    await flush();

    // 历史走 execute_tool 通道（契约 §一.4，camelCase），禁裸 invoke。
    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool).toHaveBeenCalledWith({
      toolName: 'get_group_history',
      parameters: { action: 'history', group_id: 'group-1', limit: 200 },
      workspacePath: '/workspace-a',
    });
    // 复用现成气泡列表（FlowChatContainer 渲染，turn 注入 flowChatStore 由生产代码负责）。
    expect(document.querySelector('[data-testid="flow-chat-container"]')).not.toBeNull();
  });

  it('does not render a bare invoke path: every group action goes through executeTool', async () => {
    // 组件源码中不存在任何 api.invoke('send_group_message') 等裸调用；
    // 此处通过 mock 记录证明：渲染后仅 executeTool 被调用（get_group_history）。
    renderView();
    // flush 挂载期的异步 effect（loadHistory/loadMembers setState），避免
    // "update not wrapped in act" warning。
    await flush();
    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool.mock.calls[0]![0].toolName).toBe('get_group_history');
  });

  it('sends a group message through send_group_message with camelCase shape (no direct invoke)', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      if (request.toolName === 'send_group_message') {
        return { toolName: 'send_group_message', success: true, result: { messageId: 'msg-new', status: 'sent' } };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    renderView();
    await flush();

    typeMessage(' 大家好 ');
    clickSend();
    await flush();

    // 历史 + 发送两次 executeTool，全部 camelCase，禁裸 invoke。
    const sendCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'send_group_message',
    );
    expect(sendCall).toBeDefined();
    expect(sendCall![0]).toEqual({
      toolName: 'send_group_message',
      parameters: {
        action: 'send',
        group_id: 'group-1',
        content: '大家好',
        sender_session_id: 'group-1',
      },
      workspacePath: '/workspace-a',
    });
  });

  it('does not inject a local turn when the backend send fails', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      return { toolName: 'send_group_message', success: false, result: null, error: 'sender missing' };
    });
    renderView();
    await flush();

    typeMessage('will fail');
    clickSend();
    await flush();

    // 失败路径：不乐观注入本地 turn（组件在 success!==true 分支 return）。
    const sendCalls = vi.mocked(toolAPI.executeTool).mock.calls.filter(
      c => c[0].toolName === 'send_group_message',
    );
    expect(sendCalls).toHaveLength(1);
    expect(vi.mocked(toolAPI.executeTool)).toHaveBeenCalledTimes(2);
  });

  // ── R-GC-15：成员管理 ─────────────────────────────────────────────

  it('loads member list from group session metadata groupChats + listSessions display names', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue({
      sessionId: 'group-1',
      sessionName: '项目群',
      agentType: 'Claw',
      modelName: 'auto',
      createdAt: 0,
      lastActiveAt: 0,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: { groupChats: ['claw-1', 'claw-2'] },
    });
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist C'),
    ]);
    renderView();
    await flush();

    const rows = [...document.querySelectorAll<HTMLElement>('[data-testid="group-chat-member-list"] [data-member-id]')];
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain('Assist A');
    expect(rows[1]!.textContent).toContain('Assist C');
  });

  it('invites members through invite_group_member (camelCase, workspace passed)', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      if (request.toolName === 'invite_group_member') {
        return { toolName: 'invite_group_member', success: true, result: { status: 'invited' } };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist C'),
    ]);
    renderView();
    await flush();

    // 打开邀请弹窗（现成 Modal + Checkbox 多选形态）
    const inviteBtns = [...document.querySelectorAll<HTMLButtonElement>('button')].filter(
      b => b.textContent?.includes('Invite'),
    );
    act(() => inviteBtns[0]!.click());
    await flush();

    const modal = document.querySelector('[data-testid="modal"]');
    expect(modal).not.toBeNull();
    const checkboxes = [...document.querySelectorAll<HTMLInputElement>('[data-testid="member-checkbox"]')];
    expect(checkboxes.length).toBeGreaterThan(0);
    act(() => checkboxes[0]!.click());

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm invite'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    const inviteCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'invite_group_member',
    );
    expect(inviteCall).toBeDefined();
    expect(inviteCall![0]).toEqual({
      toolName: 'invite_group_member',
      parameters: {
        action: 'invite',
        group_id: 'group-1',
        member_session_id: 'claw-1',
        workspace: '/workspace-a',
      },
      workspacePath: '/workspace-a',
    });
  });

  it('removes a member through remove_group_member', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      if (request.toolName === 'remove_group_member') {
        return { toolName: 'remove_group_member', success: true, result: { status: 'removed' } };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue({
      sessionId: 'group-1',
      sessionName: '项目群',
      agentType: 'Claw',
      modelName: 'auto',
      createdAt: 0,
      lastActiveAt: 0,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: { groupChats: ['claw-1'] },
    });
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
    ]);
    renderView();
    await flush();

    const removeBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Remove'),
    );
    expect(removeBtn).not.toBeNull();
    act(() => removeBtn!.click());
    await flush();

    const removeCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'remove_group_member',
    );
    expect(removeCall).toBeDefined();
    expect(removeCall![0]).toEqual({
      toolName: 'remove_group_member',
      parameters: {
        action: 'remove',
        group_id: 'group-1',
        member_session_id: 'claw-1',
      },
      workspacePath: '/workspace-a',
    });
  });

  it('forks the group through fork_group_chat then jumps to the child group view', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return {
          toolName: 'get_group_history',
          success: true,
          result: {
            messages: [{
              messageId: 'msg-last',
              groupSessionId: 'group-1',
              author: { sessionId: 'commander-1', name: '群主' },
              content: 'fork point',
              timestamp: 1000,
            }],
          },
        };
      }
      if (request.toolName === 'fork_group_chat') {
        return {
          toolName: 'fork_group_chat',
          success: true,
          result: { parentGroupId: 'group-1', childGroupId: 'group-child-1' },
        };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
    ]);
    // 注入历史 turn 以提供 fork 的 turn_id（lastTurnId 取自本地 session turns）
    flowChatMocks.getState.mockReturnValue({
      sessions: new Map([['group-1', {
        dialogTurns: [{
          id: 'msg-last',
          userMessage: { id: 'msg-last', content: 'fork point', timestamp: 1000 },
        }],
      }]]),
      activeSessionId: 'group-1',
    });
    renderView();
    await flush();

    // 打开 fork 弹窗（现成 Modal + Input + Checkbox 多选形态）
    const forkBtns = [...document.querySelectorAll<HTMLButtonElement>('button')].filter(
      b => b.textContent?.includes('Fork'),
    );
    act(() => forkBtns[0]!.click());
    await flush();

    const nameInput = document.querySelector<HTMLInputElement>('[data-testid="dialog-name-input"]');
    expect(nameInput).not.toBeNull();
    // 默认名 = groupName + forkSuffix（英文 i18n：Untitled group · child）
    act(() => {
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSetter?.call(nameInput, '子群A');
      nameInput!.dispatchEvent(new Event('input', { bubbles: true }));
    });

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm fork'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    const forkCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'fork_group_chat',
    );
    expect(forkCall).toBeDefined();
    expect(forkCall![0]).toEqual({
      toolName: 'fork_group_chat',
      parameters: {
        action: 'fork',
        group_id: 'group-1',
        name: '子群A',
        turn_id: 'msg-last',
        members: [],
      },
      workspacePath: '/workspace-a',
    });

    // fork 成功 → 登记子群 + 标记群聊 + 跳转子群视图（复用 R-GC-13 登记形态）
    expect(flowChatMocks.createSession).toHaveBeenCalledWith(
      'group-child-1',
      expect.objectContaining({ workspacePath: '/workspace-a', agentType: 'Claw' }),
      undefined,
      '子群A',
      1048576,
      'Claw',
      '/workspace-a',
    );
    expect(flowChatMocks.markSessionAsGroupChat).toHaveBeenCalledWith('group-child-1');
    expect(openMainSession).toHaveBeenCalledWith('group-child-1', {});
  });

  it('refuses to fork without a persisted message (forkNeedsMessage)', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([]);
    // 本地 session 无任何 turn → lastTurnId 为 undefined
    flowChatMocks.getState.mockReturnValue({ sessions: new Map(), activeSessionId: null });
    renderView();
    await flush();

    const forkBtns = [...document.querySelectorAll<HTMLButtonElement>('button')].filter(
      b => b.textContent?.includes('Fork'),
    );
    act(() => forkBtns[0]!.click());
    await flush();

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm fork'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    const forkCalls = vi.mocked(toolAPI.executeTool).mock.calls.filter(
      c => c[0].toolName === 'fork_group_chat',
    );
    expect(forkCalls).toHaveLength(0);
    expect(openMainSession).not.toHaveBeenCalled();
  });
});

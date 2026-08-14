// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/component-library', () => {
  const React = require('react');
  return {
    Modal: ({ isOpen, children }: { isOpen: boolean; children: React.ReactNode }) =>
      isOpen ? <div data-testid="modal">{children}</div> : null,
    Input: (props: { label?: string; value?: string; type?: string; min?: number; max?: number; onChange?: (e: { target: { value: string } }) => void; placeholder?: string; autoFocus?: boolean }) => (
      <input
        data-testid={String(props.label ?? '').includes('count') || String(props.label ?? '').includes('Count') ? 'member-count-input' : 'group-name-input'}
        aria-label={props.label}
        type={props.type ?? 'text'}
        min={props.min}
        max={props.max}
        placeholder={props.placeholder}
        value={props.value ?? ''}
        onChange={props.onChange}
        autoFocus={props.autoFocus}
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

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
  },
}));

import CreateGroupChatDialog from './CreateGroupChatDialog';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';

describe('CreateGroupChatDialog (R-GC-13 / R-GC-28)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.mocked(toolAPI.executeTool).mockReset();
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
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSetter?.call(input, value);
      input!.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };

  const setMemberCount = (value: string) => {
    const input = document.querySelector<HTMLInputElement>('[data-testid="member-count-input"]');
    expect(input).not.toBeNull();
    act(() => {
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

  it('creates the group through toolAPI.executeTool with camelCase shape (no direct invoke)', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-1' },
      error: null,
      validation_error: null,
      duration_ms: 1,
    });
    const { onCreated } = renderDialog();

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

  it('sends count-driven member placeholders (R-GC-28: backend creates fresh UUID sessions)', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-9' },
    });
    const { onCreated } = renderDialog();

    setGroupName('群A');
    setMemberCount('3');
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledWith(expect.objectContaining({
      parameters: {
        action: 'create',
        name: '群A',
        members: ['member-1', 'member-2', 'member-3'],
        workspace: '/workspace-a',
      },
    }));
    expect(onCreated).toHaveBeenCalledWith('group-9', '群A');
  });

  it('omits workspace parameter when workspacePath is empty (backend default fallback, R-GC-17)', async () => {
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

  it('surfaces backend failure without calling onCreated', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: false,
      result: null,
      error: 'name is required for create',
      validation_error: null,
      duration_ms: 1,
    });
    const { onCreated } = renderDialog();

    setGroupName('空');
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(onCreated).not.toHaveBeenCalled();
  });
});

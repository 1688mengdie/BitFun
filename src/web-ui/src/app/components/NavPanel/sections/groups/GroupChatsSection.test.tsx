// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GroupChatsSection } from './GroupChatsSection';
import { useGroupChatStore } from '../../../../../flow_chat/store/groupChatStore';
import type { GroupChatMember, GroupChatRoom } from '../../../../../flow_chat/types/flow-chat';

let mockCurrentWorkspace: { rootPath: string } | null = { rootPath: '/ws' };
vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({ currentWorkspace: mockCurrentWorkspace }),
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke: vi.fn() },
}));

vi.mock('@/component-library/components/ConfirmDialog/confirmService', () => ({
  confirmWarning: vi.fn(),
}));

// P2-13: Virtuoso needs real layout measurement (ResizeObserver + clientHeight)
// that jsdom cannot provide. Stub with a full render so behavior assertions
// stay meaningful; windowing itself is a browser-layout concern.
vi.mock('react-virtuoso', () => ({
  Virtuoso: (props: { data?: unknown[]; itemContent?: (index: number, item: unknown) => React.ReactNode; computeItemKey?: (index: number, item: unknown) => string | number }) => {
    const items = (props.data ?? []) as { roomId: string; key: string }[];
    return React.createElement('div', null, items.map((item, index) =>
      React.createElement(
        'div',
        { key: props.computeItemKey ? props.computeItemKey(index, item) : (item as { roomId: string }).roomId },
        props.itemContent ? props.itemContent(index, item) : null,
      ),
    ));
  },
}));

import { api } from '@/infrastructure/api/service-api/ApiClient';
const mockedInvoke = vi.mocked(api.invoke);

import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
const mockedConfirm = vi.mocked(confirmWarning);

function sampleMember(sessionId: string): GroupChatMember {
  return {
    sessionId,
    role: 'member',
    joinedAt: 1,
    agentType: 'Claw',
    displayName: `Assistant ${sessionId}`,
  };
}

function sampleRoom(roomId: string, name: string, mode: GroupChatRoom['mode'] = 'free'): GroupChatRoom {
  return {
    schemaVersion: 1,
    roomId,
    name,
    owner: { kind: 'master' },
    mode,
    roundRobinCursor: 0,
    createdAt: 1,
    lastActiveAt: mode === 'round_robin' ? 3 : 1,
    status: 'active',
    memberLimit: 50,
  };
}

let container: HTMLDivElement;
let root: Root;

function renderSection() {
  act(() => {
    root.render(<GroupChatsSection workspacePath="/ws" isVisible />);
  });
}

describe('GroupChatsSection', () => {
  beforeEach(() => {
    mockCurrentWorkspace = { rootPath: '/ws' };
    container = document.createElement('div');
    // P2-13: the virtualized list needs a measurable viewport height — without
    // it Virtuoso renders zero rows in jsdom.
    container.style.height = '600px';
    document.body.appendChild(container);
    root = createRoot(container);
    mockedConfirm.mockReset();
    mockedInvoke.mockReset();
    // GroupChatsSection's mount effect calls loadRooms/loadMembers which set
    // store state on IPC resolve. Keep those promises pending so the effect's
    // state updates never land outside act() (they would trigger "not wrapped
    // in act(...)" warnings); the tests seed the store directly instead.
    mockedInvoke.mockReturnValue(new Promise(() => {}));
    useGroupChatStore.setState({
      rooms: new Map(),
      activeRoomId: null,
      members: new Map(),
      messages: new Map(),
      mode: 'free',
      roundRobinCursor: 0,
      workspacePath: '',
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('shows the empty state when no rooms exist', () => {
    renderSection();
    expect(container.querySelector('[data-bf-part="empty"]')).toBeTruthy();
    expect(container.querySelector('[data-bf-part="items"]')).toBeNull();
  });

  it('renders room rows with name, member count, and mode badge', () => {
    useGroupChatStore.setState({
      rooms: new Map([
        ['room-1', sampleRoom('room-1', 'Alpha', 'free')],
        ['room-2', sampleRoom('room-2', 'Beta', 'round_robin')],
      ]),
      // P1-2 修复：成员数来自 members Map（真实成员数），非 memberLimit。
      members: new Map([
        ['room-1', [sampleMember('m-1'), sampleMember('m-2')]],
        ['room-2', [sampleMember('m-3')]],
      ]),
    });
    renderSection();

    const section = container.querySelector('[data-bf-part="root"]') as HTMLElement;
    section.style.height = '600px';
    act(() => {
      section.dispatchEvent(new Event('resize'));
    });

    const items = Array.from(container.querySelectorAll('[data-bf-part="item"]'));
    expect(items.length).toBe(2);
    // 按 lastActiveAt 降序：Beta(3) 在 Alpha(1) 前。
    expect(items[0].textContent).toContain('Beta');
    expect(items[0].textContent).toContain('1');
    expect(items[1].textContent).toContain('Alpha');
    expect(items[1].textContent).toContain('2');
  });

  it('activates the room on click', () => {
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
    });
    renderSection();

    const item = container.querySelector('[data-bf-part="item"]') as HTMLElement;
    act(() => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(useGroupChatStore.getState().activeRoomId).toBe('room-1');
  });

  it('deletes a room after confirmation (P0-3)', async () => {
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
      activeRoomId: 'room-1',
    });
    mockedConfirm.mockResolvedValueOnce(true);
    // Only the delete IPC resolves; loadRooms/loadMembers keep pending so the
    // mount effect does not produce act() warnings.
    mockedInvoke.mockImplementation((command: string) => {
      if (command === 'group_chat_delete') return Promise.resolve(undefined);
      return new Promise(() => {});
    });
    renderSection();

    const deleteButton = container.querySelector('[data-bf-action="delete-room"]') as HTMLElement;
    expect(deleteButton).toBeTruthy();
    await act(async () => {
      deleteButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(mockedConfirm).toHaveBeenCalled();
    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_delete', expect.any(Object));
    expect(useGroupChatStore.getState().rooms.has('room-1')).toBe(false);
    expect(useGroupChatStore.getState().activeRoomId).toBeNull();
  });

  it('keeps the room when the delete confirmation is cancelled', async () => {
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
    });
    mockedConfirm.mockResolvedValueOnce(false);
    renderSection();

    const deleteButton = container.querySelector('[data-bf-action="delete-room"]') as HTMLElement;
    await act(async () => {
      deleteButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(useGroupChatStore.getState().rooms.has('room-1')).toBe(true);
  });

  // P0 regression (2026-08-14, owner report: newly created group chat does not
  // show in the list). Candidate A: the workspace context is initially null
  // (workspaceManager loads asynchronously), so effectiveWorkspacePath is ''
  // on first mount. The section must NOT skip loading forever - once the
  // workspace resolves it must load rooms.
  it('loads rooms when the workspace becomes ready after mount (delayed workspace)', async () => {
    mockCurrentWorkspace = null; // workspace not ready on first mount
    mockedInvoke.mockImplementation((command: string) => {
      if (command === 'group_chat_list') {
        return Promise.resolve([sampleRoom('room-late', 'LateRoom')]);
      }
      return new Promise(() => {});
    });
    renderSection();

    // First mount with no workspace: no load yet, empty state shown.
    expect(container.querySelector('[data-bf-part="empty"]')).toBeTruthy();

    // Workspace resolves; effectiveWorkspacePath changes -> effect re-runs.
    mockCurrentWorkspace = { rootPath: '/ws' };
    await act(async () => {
      root.render(<GroupChatsSection workspacePath="/ws" isVisible />);
    });

    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_list', {
      request: { workspace_path: '/ws' },
    });
    const item = container.querySelector('[data-bf-part="item"]');
    expect(item?.textContent).toContain('LateRoom');
  });

  // Candidate B: a room created while the section is already mounted must
  // appear immediately (createRoom sets the store + re-syncs the list; the
  // section subscribes to rooms). This guards the "created but list stays
  // empty" regression.
  it('renders a newly created room without requiring remount (reactive rooms)', async () => {
    mockedInvoke.mockImplementation((command: string) => {
      if (command === 'group_chat_create') {
        return Promise.resolve(sampleRoom('room-fresh', 'FreshRoom'));
      }
      if (command === 'group_chat_list') {
        return Promise.resolve([sampleRoom('room-fresh', 'FreshRoom')]);
      }
      return new Promise(() => {});
    });
    renderSection();

    await act(async () => {
      await useGroupChatStore.getState().createRoom('FreshRoom', { kind: 'master' }, ['m-1'], 'free');
    });

    const item = container.querySelector('[data-bf-part="item"]');
    expect(item?.textContent).toContain('FreshRoom');
  });
});

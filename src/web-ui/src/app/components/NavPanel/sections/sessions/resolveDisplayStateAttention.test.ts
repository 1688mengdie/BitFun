import { describe, expect, it } from 'vitest';
import { SessionDisplayState } from '../../../../../flow_chat/state-machine/types';
import type { Session } from '../../../../../flow_chat/types/flow-chat';
import { resolveDisplayStateAttention } from './resolveDisplayStateAttention';

const createSession = (displayState?: SessionDisplayState): Session => ({
  sessionId: 'session-1',
  title: 'Session 1',
  dialogTurns: [],
  status: 'idle',
  config: { agentType: 'agentic' },
  createdAt: 1,
  lastActiveAt: 1,
  error: null,
  isHistorical: false,
  todos: [],
  maxContextTokens: 1048576,
  mode: 'agentic',
  workspacePath: 'D:/workspace/BitFun',
  isTransient: false,
  displayState,
});

describe('resolveDisplayStateAttention', () => {
  it('R-12: maps VIEWED to undefined (green dot stays cleared)', () => {
    expect(resolveDisplayStateAttention(createSession(SessionDisplayState.VIEWED))).toBeUndefined();
  });

  it('maps COMPLETED to completed (green dot)', () => {
    expect(resolveDisplayStateAttention(createSession(SessionDisplayState.COMPLETED))).toBe('completed');
  });

  it('maps PENDING_ATTENTION to ask_user', () => {
    expect(resolveDisplayStateAttention(createSession(SessionDisplayState.PENDING_ATTENTION))).toBe('ask_user');
  });

  it('maps INTERRUPTED to interrupted', () => {
    expect(resolveDisplayStateAttention(createSession(SessionDisplayState.INTERRUPTED))).toBe('interrupted');
  });

  it('maps PROCESSING / HUNG / STANDBY / undefined to undefined (no dot)', () => {
    expect(resolveDisplayStateAttention(createSession(SessionDisplayState.PROCESSING))).toBeUndefined();
    expect(resolveDisplayStateAttention(createSession(SessionDisplayState.HUNG))).toBeUndefined();
    expect(resolveDisplayStateAttention(createSession(SessionDisplayState.STANDBY))).toBeUndefined();
    // EphemeralChild sessions carry no backend projection.
    expect(resolveDisplayStateAttention(createSession(undefined))).toBeUndefined();
  });
});

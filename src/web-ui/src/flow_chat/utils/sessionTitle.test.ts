import { describe, expect, it } from 'vitest';
import type { SessionMetadata } from '@/shared/types/session-history';
import {
  deriveSessionTitleStateFromMetadata,
  freezeSessionTitleState,
  resolvePersistedSessionTitle,
  resolveSessionTitle,
} from './sessionTitle';

function createTranslator(locale: 'en' | 'zh') {
  return (key: string, options?: Record<string, unknown>) => {
    const count = options?.count;
    if (key === 'flow-chat:session.newCodeWithIndex') {
      return locale === 'zh' ? `新建代码会话 ${count}` : `New Code Session ${count}`;
    }
    if (key === 'flow-chat:session.new') {
      return locale === 'zh' ? '新建会话' : 'New Session';
    }
    return key;
  };
}

describe('sessionTitle', () => {
  it('renders untouched default session titles from the current locale', () => {
    const session = {
      title: 'New Code Session 2',
      titleSource: 'i18n',
      titleI18nKey: 'flow-chat:session.newCodeWithIndex',
      titleI18nParams: { count: 2 },
    } as const;

    expect(resolveSessionTitle(session, createTranslator('en'))).toBe('New Code Session 2');
    expect(resolveSessionTitle(session, createTranslator('zh'))).toBe('新建代码会话 2');
  });

  it('keeps generated titles fixed after the first message', () => {
    const frozen = freezeSessionTitleState('Fix flaky test');

    expect(resolveSessionTitle(frozen, createTranslator('en'))).toBe('Fix flaky test');
    expect(resolveSessionTitle(frozen, createTranslator('zh'))).toBe('Fix flaky test');
  });

  it('restores persisted locale-aware titles only for untouched sessions', () => {
    const untouchedMetadata: SessionMetadata = {
      sessionId: 'session-1',
      sessionName: 'New Code Session 3',
      agentType: 'agentic',
      modelName: 'auto',
      createdAt: 1,
      lastActiveAt: 2,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: {
        kind: 'normal',
        titleSource: 'i18n',
        titleKey: 'flow-chat:session.newCodeWithIndex',
        titleParams: { count: 3 },
      },
    };
    const activeMetadata: SessionMetadata = {
      ...untouchedMetadata,
      turnCount: 1,
      sessionName: 'Fix flaky test',
    };

    expect(deriveSessionTitleStateFromMetadata(untouchedMetadata)).toMatchObject({
      titleSource: 'i18n',
      titleI18nKey: 'flow-chat:session.newCodeWithIndex',
      titleI18nParams: { count: 3 },
    });
    expect(resolvePersistedSessionTitle(untouchedMetadata, createTranslator('zh'))).toBe(
      '新建代码会话 3',
    );

    expect(deriveSessionTitleStateFromMetadata(activeMetadata)).toMatchObject({
      title: 'Fix flaky test',
      titleSource: 'text',
      titleI18nKey: undefined,
    });
    expect(resolvePersistedSessionTitle(activeMetadata, createTranslator('zh'))).toBe(
      'Fix flaky test',
    );
  });
});

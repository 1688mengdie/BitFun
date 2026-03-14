import type { Session } from '../types/flow-chat';

export function getSessionSortTimestamp(session: Pick<Session, 'createdAt' | 'lastFinishedAt'>): number {
  return session.lastFinishedAt ?? session.createdAt;
}

export function compareSessionsForDisplay(
  a: Pick<Session, 'sessionId' | 'createdAt' | 'lastFinishedAt'>,
  b: Pick<Session, 'sessionId' | 'createdAt' | 'lastFinishedAt'>
): number {
  const timestampDiff = getSessionSortTimestamp(b) - getSessionSortTimestamp(a);
  if (timestampDiff !== 0) {
    return timestampDiff;
  }

  const createdAtDiff = b.createdAt - a.createdAt;
  if (createdAtDiff !== 0) {
    return createdAtDiff;
  }

  return a.sessionId.localeCompare(b.sessionId);
}

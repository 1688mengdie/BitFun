import React, { useEffect, useCallback, useState } from 'react';
import { RemoteSessionManager, SessionInfo } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';

interface SessionListPageProps {
  sessionMgr: RemoteSessionManager;
  onSelectSession: (sessionId: string) => void;
}

function formatTime(unixStr: string): string {
  const ts = parseInt(unixStr, 10);
  if (!ts || isNaN(ts)) return '';
  const date = new Date(ts * 1000);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return 'just now';
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDay = Math.floor(diffHr / 24);
  if (diffDay < 7) return `${diffDay}d ago`;
  return date.toLocaleDateString();
}

function agentLabel(agentType: string): string {
  switch (agentType) {
    case 'code':
    case 'agentic':
      return 'Code';
    case 'cowork':
    case 'Cowork':
      return 'Cowork';
    default:
      return agentType || 'Default';
  }
}

const SessionListPage: React.FC<SessionListPageProps> = ({ sessionMgr, onSelectSession }) => {
  const { sessions, setSessions, setError } = useMobileStore();
  const [creating, setCreating] = useState(false);
  const [loading, setLoading] = useState(false);
  const [showNewMenu, setShowNewMenu] = useState(false);

  const loadSessions = useCallback(async () => {
    setLoading(true);
    try {
      const list = await sessionMgr.listSessions();
      // Sort by updated_at descending (most recent first)
      list.sort((a, b) => parseInt(b.updated_at, 10) - parseInt(a.updated_at, 10));
      setSessions(list);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, [sessionMgr, setSessions, setError]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleCreate = async (agentType: string) => {
    if (creating) return;
    setCreating(true);
    setShowNewMenu(false);
    try {
      const id = await sessionMgr.createSession(agentType);
      await loadSessions();
      onSelectSession(id);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="session-list">
      <div className="session-list__header">
        <h1>BitFun Sessions</h1>
        <div className="session-list__new-wrapper">
          <button
            className="session-list__new-btn"
            onClick={() => setShowNewMenu(!showNewMenu)}
            disabled={creating}
            style={{ opacity: creating ? 0.5 : 1 }}
          >
            {creating ? 'Creating...' : '+ New'}
          </button>
          {showNewMenu && (
            <div className="session-list__new-menu">
              <button
                className="session-list__menu-item"
                onClick={() => handleCreate('code')}
              >
                <span className="session-list__menu-icon">{'</>'}</span>
                Code Session
              </button>
              <button
                className="session-list__menu-item"
                onClick={() => handleCreate('cowork')}
              >
                <span className="session-list__menu-icon">{'<>'}</span>
                Cowork Session
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="session-list__items">
        {loading && sessions.length === 0 && (
          <div className="session-list__empty">Loading sessions...</div>
        )}
        {!loading && sessions.length === 0 && (
          <div className="session-list__empty">No sessions yet. Create one to get started.</div>
        )}
        {sessions.map((s) => (
          <div
            key={s.session_id}
            className="session-list__item"
            onClick={() => onSelectSession(s.session_id)}
          >
            <div className="session-list__item-top">
              <div className="session-list__item-name">{s.name || 'Untitled Session'}</div>
              <span className={`session-list__agent-badge session-list__agent-badge--${s.agent_type}`}>
                {agentLabel(s.agent_type)}
              </span>
            </div>
            <div className="session-list__item-meta">
              <span>{s.message_count} messages</span>
              <span className="session-list__item-time">{formatTime(s.updated_at)}</span>
            </div>
          </div>
        ))}
      </div>

      <button className="session-list__refresh" onClick={loadSessions} disabled={loading}>
        {loading ? 'Refreshing...' : 'Refresh'}
      </button>
    </div>
  );
};

export default SessionListPage;

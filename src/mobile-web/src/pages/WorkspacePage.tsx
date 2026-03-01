import React, { useEffect, useState, useCallback } from 'react';
import {
  RemoteSessionManager,
  WorkspaceInfo,
  RecentWorkspaceEntry,
} from '../services/RemoteSessionManager';

interface WorkspacePageProps {
  sessionMgr: RemoteSessionManager;
  onReady: () => void;
}

const WorkspacePage: React.FC<WorkspacePageProps> = ({ sessionMgr, onReady }) => {
  const [workspaceInfo, setWorkspaceInfo] = useState<WorkspaceInfo | null>(null);
  const [recentWorkspaces, setRecentWorkspaces] = useState<RecentWorkspaceEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showRecent, setShowRecent] = useState(false);

  const loadWorkspaceInfo = useCallback(async () => {
    try {
      const info = await sessionMgr.getWorkspaceInfo();
      setWorkspaceInfo(info);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, [sessionMgr]);

  const loadRecentWorkspaces = useCallback(async () => {
    try {
      const list = await sessionMgr.listRecentWorkspaces();
      setRecentWorkspaces(list);
    } catch (e: any) {
      setError(e.message);
    }
  }, [sessionMgr]);

  useEffect(() => {
    loadWorkspaceInfo();
  }, [loadWorkspaceInfo]);

  const handleShowRecent = async () => {
    setShowRecent(true);
    await loadRecentWorkspaces();
  };

  const handleSelectWorkspace = async (path: string) => {
    if (switching) return;
    setSwitching(true);
    setError(null);
    try {
      const result = await sessionMgr.setWorkspace(path);
      if (result.success) {
        await loadWorkspaceInfo();
        setShowRecent(false);
      } else {
        setError(result.error || 'Failed to set workspace');
      }
    } catch (e: any) {
      setError(e.message);
    } finally {
      setSwitching(false);
    }
  };

  const handleContinue = () => {
    onReady();
  };

  if (loading) {
    return (
      <div className="workspace-page">
        <div className="workspace-page__loading">
          <div className="spinner" />
          <span>Loading workspace info...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="workspace-page">
      <div className="workspace-page__header">
        <h1>Workspace</h1>
      </div>

      <div className="workspace-page__content">
        {workspaceInfo?.has_workspace ? (
          <div className="workspace-page__current">
            <div className="workspace-page__current-label">Current Workspace</div>
            <div className="workspace-page__current-card">
              <div className="workspace-page__project-name">
                {workspaceInfo.project_name || 'Unknown Project'}
              </div>
              <div className="workspace-page__project-path">{workspaceInfo.path}</div>
              {workspaceInfo.git_branch && (
                <div className="workspace-page__git-branch">
                  <span className="workspace-page__branch-icon">⎇</span>
                  {workspaceInfo.git_branch}
                </div>
              )}
            </div>
            <div className="workspace-page__actions">
              <button className="workspace-page__btn workspace-page__btn--primary" onClick={handleContinue}>
                Continue
              </button>
              <button className="workspace-page__btn workspace-page__btn--secondary" onClick={handleShowRecent}>
                Switch Workspace
              </button>
            </div>
          </div>
        ) : (
          <div className="workspace-page__no-workspace">
            <div className="workspace-page__no-workspace-icon">📂</div>
            <div className="workspace-page__no-workspace-text">
              No workspace is currently open on the desktop.
            </div>
            <div className="workspace-page__no-workspace-hint">
              Select a recent workspace below, or open one on the desktop first.
            </div>
            {!showRecent && (
              <button className="workspace-page__btn workspace-page__btn--primary" onClick={handleShowRecent}>
                Select Workspace
              </button>
            )}
          </div>
        )}

        {showRecent && (
          <div className="workspace-page__recent">
            <div className="workspace-page__recent-label">Recent Workspaces</div>
            {recentWorkspaces.length === 0 ? (
              <div className="workspace-page__recent-empty">
                No recent workspaces found. Please open a workspace on the desktop first.
              </div>
            ) : (
              <div className="workspace-page__recent-list">
                {recentWorkspaces.map((ws) => (
                  <button
                    key={ws.path}
                    className="workspace-page__recent-item"
                    onClick={() => handleSelectWorkspace(ws.path)}
                    disabled={switching}
                  >
                    <div className="workspace-page__recent-item-name">{ws.name}</div>
                    <div className="workspace-page__recent-item-path">{ws.path}</div>
                  </button>
                ))}
              </div>
            )}
            {workspaceInfo?.has_workspace && (
              <button
                className="workspace-page__btn workspace-page__btn--secondary"
                onClick={() => setShowRecent(false)}
              >
                Cancel
              </button>
            )}
          </div>
        )}

        {switching && (
          <div className="workspace-page__switching">
            <div className="spinner" />
            <span>Opening workspace...</span>
          </div>
        )}

        {error && <div className="workspace-page__error">{error}</div>}
      </div>
    </div>
  );
};

export default WorkspacePage;

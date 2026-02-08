import React, { useState } from 'react';
import { FolderOpen, Clock, FileText, Code, Folder } from 'lucide-react';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { WorkspaceInfo, WorkspaceType } from '../../../shared/types';
import { Modal } from '@/component-library';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import './WorkspaceManager.css';

const log = createLogger('WorkspaceManager');

interface WorkspaceManagerProps {
  isVisible: boolean;
  onClose: () => void;
  onWorkspaceSelect?: (workspace: WorkspaceInfo) => void;
}

/**
 * Workspace management component.
 * Displays current workspace status and recent workspaces.
 */
const WorkspaceManager: React.FC<WorkspaceManagerProps> = ({
  isVisible,
  onClose,
  onWorkspaceSelect
}) => {
  const {
    currentWorkspace,
    recentWorkspaces,
    loading,
    error,
    switchWorkspace,
    closeWorkspace,
    scanWorkspaceInfo
  } = useWorkspaceContext();

  const [scanning, setScanning] = useState(false);

  const handleWorkspaceSelect = async (workspace: WorkspaceInfo) => {
    try {
      await switchWorkspace(workspace);
      onWorkspaceSelect?.(workspace);
      onClose();
    } catch (err) {
      log.error('Failed to switch workspace', { workspaceId: workspace.id, error: err });
    }
  };

  const handleCloseWorkspace = async () => {
    try {
      await closeWorkspace();
    } catch (err) {
      log.error('Failed to close workspace', err);
    }
  };

  const handleScanWorkspace = async () => {
    try {
      setScanning(true);
      await scanWorkspaceInfo();
    } catch (err) {
      log.error('Failed to scan workspace', err);
    } finally {
      setScanning(false);
    }
  };

  const getWorkspaceIcon = (type: WorkspaceType) => {
    switch (type) {
      case WorkspaceType.SingleProject:
        return <Code size={16} />;
      case WorkspaceType.Documentation:
        return <FileText size={16} />;
      case WorkspaceType.MultiProject:
        return <Folder size={16} />;
      default:
        return <FolderOpen size={16} />;
    }
  };

  const formatDate = (dateStr: string) => {
    try {
      return i18nService.formatDate(new Date(dateStr), {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
      });
    } catch {
      return dateStr;
    }
  };

  return (
    <Modal
      isOpen={isVisible}
      onClose={onClose}
      title="Workspace Status"
      size="medium"
    >
      <div className="workspace-manager">
        {error && (
          <div className="error-message">
            <span>Error: {error}</span>
          </div>
        )}

        <div className="current-workspace-section">
          <h3>Current Workspace</h3>
          {currentWorkspace ? (
            <div className="workspace-card current">
              <div className="workspace-header">
                <div className="workspace-icon">
                  {getWorkspaceIcon(currentWorkspace.workspaceType)}
                </div>
                <div className="workspace-info">
                  <div className="workspace-name">{currentWorkspace.name}</div>
                  <div className="workspace-path">{currentWorkspace.rootPath}</div>
                  <div className="workspace-meta">
                    <span className="workspace-type">{currentWorkspace.workspaceType}</span>
                    {currentWorkspace.lastAccessed && (
                      <span className="workspace-time">
                        <Clock size={12} />
                        {formatDate(currentWorkspace.lastAccessed)}
                      </span>
                    )}
                  </div>
                </div>
              </div>
              
              <div className="workspace-actions">
                <button
                  className="btn btn-secondary btn-small"
                  onClick={handleScanWorkspace}
                  disabled={scanning}
                >
                  {scanning ? 'Scanning...' : 'Rescan'}
                </button>
                <button
                  className="btn btn-danger btn-small"
                  onClick={handleCloseWorkspace}
                  disabled={loading}
                >
                  Close Workspace
                </button>
              </div>

              {currentWorkspace.statistics && (
                <div className="workspace-stats">
                  <div className="stat-item">
                    <span className="stat-label">Files:</span>
                    <span className="stat-value">{currentWorkspace.statistics.totalFiles}</span>
                  </div>
                  <div className="stat-item">
                    <span className="stat-label">Lines:</span>
                    <span className="stat-value">{currentWorkspace.statistics.totalLines?.toLocaleString()}</span>
                  </div>
                  <div className="stat-item">
                    <span className="stat-label">Total Size:</span>
                    <span className="stat-value">{(currentWorkspace.statistics.totalSize / 1024 / 1024).toFixed(2)} MB</span>
                  </div>
                </div>
              )}
            </div>
          ) : (
            <div className="no-workspace">
              <FolderOpen size={48} />
              <p>No workspace is currently open</p>
            </div>
          )}
        </div>

        <div className="recent-workspaces-section">
          <h3>Recent Workspaces</h3>
          {recentWorkspaces.length > 0 ? (
            <div className="workspace-list">
              {recentWorkspaces.map((workspace) => (
                <div
                  key={workspace.id}
                  className="workspace-card recent"
                  onClick={() => handleWorkspaceSelect(workspace)}
                >
                  <div className="workspace-header">
                    <div className="workspace-icon">
                      {getWorkspaceIcon(workspace.workspaceType)}
                    </div>
                    <div className="workspace-info">
                      <div className="workspace-name">{workspace.name}</div>
                      <div className="workspace-path">{workspace.rootPath}</div>
                      <div className="workspace-meta">
                        <span className="workspace-type">{workspace.workspaceType}</span>
                        {workspace.lastAccessed && (
                          <span className="workspace-time">
                            <Clock size={12} />
                            {formatDate(workspace.lastAccessed)}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="no-recent">
              <p>No recent workspaces</p>
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
};

export default WorkspaceManager;

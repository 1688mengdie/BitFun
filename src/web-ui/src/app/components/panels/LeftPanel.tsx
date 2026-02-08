/**
 * Left panel component
 * Singleton display panel - only displays one active panel content
 */

import React, { memo } from 'react';
import { PanelType } from '../../types';

import { GitPanel } from '../../../tools/git';
import { ProjectContextPanel } from '../../../tools/project-context';
import { FilesPanel } from './';
import SessionsPanel from './SessionsPanel';
import TerminalSessionsPanel from './TerminalSessionsPanel';

import './LeftPanel.scss';

interface LeftPanelProps {
  activeTab: PanelType;
  width: number;
  isFullscreen: boolean;
  workspacePath?: string;
  onSwitchTab: (tab: PanelType) => void;
  isDragging?: boolean;
}

const LeftPanel: React.FC<LeftPanelProps> = ({
  activeTab,
  width: _width,
  isFullscreen,
  workspacePath,
  onSwitchTab: _onSwitchTab,
  isDragging: _isDragging = false
}) => {
  return (
    <div 
      className="bitfun-left-panel__content"
      data-fullscreen={isFullscreen}
    >
      <div style={{ display: activeTab === 'sessions' ? 'block' : 'none', height: '100%' }}>
        <SessionsPanel />
      </div>

      <div style={{ display: activeTab === 'files' ? 'block' : 'none', height: '100%' }}>
        <FilesPanel 
          workspacePath={workspacePath}
        />
      </div>

      <div style={{ display: activeTab === 'git' ? 'block' : 'none', height: '100%' }}>
        <GitPanel 
          workspacePath={workspacePath}
          isActive={activeTab === 'git'}
        />
      </div>

      <div style={{ display: activeTab === 'project-context' ? 'block' : 'none', height: '100%' }}>
        <ProjectContextPanel 
          workspacePath={workspacePath || ''}
          isActive={activeTab === 'project-context'}
        />
      </div>

      <div style={{ display: activeTab === 'terminal' ? 'block' : 'none', height: '100%' }}>
        <TerminalSessionsPanel />
      </div>
    </div>
  );
};

export default memo(LeftPanel);

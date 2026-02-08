/**
 * Center panel component
 * Specifically for displaying FlowChat
 */

import React, { useCallback, memo } from 'react';
import { FlowChatContainer } from '../../../flow_chat';
import { useCanvasStore } from './content-canvas/stores/canvasStore';
import type { LineRange } from '@/component-library';
import path from 'path-browserify';
import { createLogger } from '@/shared/utils/logger';

import './CenterPanel.scss';

const log = createLogger('CenterPanel');


interface CenterPanelProps {
  width: number;
  isFullscreen: boolean;
  workspacePath?: string;
  isDragging?: boolean;
}

const CenterPanelInner: React.FC<CenterPanelProps> = ({
  width: _width,
  isFullscreen,
  workspacePath,
  isDragging: _isDragging = false
}) => {
  const addTab = useCanvasStore(state => state.addTab);

  const handleFileViewRequest = useCallback(async (
    filePath: string,
    fileName: string,
    lineRange?: LineRange
  ) => {
    log.info('File view request', { filePath, fileName, lineRange, workspacePath });

    if (!filePath) {
      log.warn('Invalid file path');
      return;
    }

    let absoluteFilePath = filePath;
    const isWindowsAbsolutePath = /^[A-Za-z]:[\\/]/.test(filePath);

    if (!isWindowsAbsolutePath && !path.isAbsolute(filePath) && workspacePath) {
      absoluteFilePath = path.join(workspacePath, filePath);
      log.debug('Converting relative path to absolute', {
        relative: filePath,
        absolute: absoluteFilePath
      });
    }

    const { fileTabManager } = await import('@/shared/services/FileTabManager');
    fileTabManager.openFile({
      filePath: absoluteFilePath,
      jumpToRange: lineRange,
      mode: 'agent'
    });
  }, [workspacePath]);

  return (
    <div 
      className="bitfun-center-panel__content"
      data-fullscreen={isFullscreen}
    >
      <FlowChatContainer 
        className="bitfun-center-panel__chat-container"
        onOpenVisualization={(type, data) => {
          log.info('Opening visualization', { type, data });
        }}
        onFileViewRequest={handleFileViewRequest}
        onTabOpen={(tabInfo) => {
          log.info('Opening tab', { tabInfo });
          if (tabInfo && tabInfo.type) {
            addTab({
              type: tabInfo.type,
              title: tabInfo.title || 'New Tab',
              data: tabInfo.data,
              metadata: tabInfo.metadata
            });
          }
        }}
        onSwitchToChatPanel={() => {}}
        config={{
          enableMarkdown: true,
          autoScroll: true,
          showTimestamps: false,
          theme: 'auto'
        }}
      />
    </div>
  );
};

const CenterPanel = memo(CenterPanelInner);
CenterPanel.displayName = 'CenterPanel';

export default CenterPanel;


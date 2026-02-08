import React, { useState, useEffect } from 'react';
import { ChevronRight, ChevronDown, FolderOpen, FileText } from 'lucide-react';
import { FileTreeNodeProps, FileSystemNode } from '../types';
import { getFileIcon, getFileIconClass } from '../utils/fileIcons';
import { getCompressionTooltip } from '../utils/pathCompression';
import { dragManager } from '../../../shared/services/DragManager';
import { fileTreeDragSource } from '../../../shared/context-system/drag-drop/FileTreeDragSource';
import { Input } from '../../../component-library/components/Input';
import { GitStatusIndicator } from './GitStatusIndicator';
import { useI18n } from '@/infrastructure/i18n';

/**
 * Get the Git status to display for a directory.
 * Single status: show that status. Multiple: show 'modified'.
 */
function getDirectoryGitStatus(
  childrenGitStatuses?: Set<'untracked' | 'modified' | 'added' | 'deleted' | 'renamed' | 'conflicted' | 'staged'>
): 'untracked' | 'modified' | 'added' | 'deleted' | 'renamed' | 'conflicted' | 'staged' | undefined {
  if (!childrenGitStatuses || childrenGitStatuses.size === 0) {
    return undefined;
  }
  
  if (childrenGitStatuses.size === 1) {
    return Array.from(childrenGitStatuses)[0];
  }
  
  return 'modified';
}

interface ExtendedFileTreeNodeProps extends FileTreeNodeProps {
  selectedFile?: string;
  expandedFolders?: Set<string>;
}

/**
 * Calculate node depth relative to workspace.
 * Ensures indentation based on actual path level, not render tree level.
 */
function getPathDepth(nodePath: string, workspacePath?: string): number {
  if (!workspacePath) {
    const normalized = nodePath.replace(/\\/g, '/');
    return normalized.split('/').filter(Boolean).length - 1;
  }
  
  const normalizedNode = nodePath.replace(/\\/g, '/').toLowerCase();
  const normalizedWorkspace = workspacePath.replace(/\\/g, '/').toLowerCase();
  
  let relativePath = normalizedNode;
  if (normalizedNode.startsWith(normalizedWorkspace)) {
    relativePath = normalizedNode.slice(normalizedWorkspace.length);
  }
  
  const segments = relativePath.replace(/^\//, '').split('/').filter(Boolean);
  return segments.length;
}

interface RenameInputProps {
  node: FileSystemNode;
  onRename: (newName: string) => void;
  onCancel?: () => void;
}

const RenameInput: React.FC<RenameInputProps> = ({ node, onRename, onCancel }) => {
  const [value, setValue] = useState(node.name);

  useEffect(() => {
    const timer = setTimeout(() => {
      const input = document.querySelector('.bitfun-file-explorer__rename-input-wrapper input') as HTMLInputElement;
      if (input) {
        input.focus();
        const dotIndex = node.name.lastIndexOf('.');
        if (dotIndex > 0 && !node.isDirectory) {
          input.setSelectionRange(0, dotIndex);
        } else {
          input.select();
        }
      }
    }, 10);
    
    return () => clearTimeout(timer);
  }, [node.name, node.isDirectory]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      const newName = value.trim();
      if (newName && newName !== node.name) {
        onRename(newName);
      } else {
        onCancel?.();
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onCancel?.();
    }
  };

  const handleBlur = () => {
    const newName = value.trim();
    if (newName && newName !== node.name) {
      onRename(newName);
    } else {
      onCancel?.();
    }
  };

  return (
    <div className="bitfun-file-explorer__rename-input-wrapper" onClick={(e) => e.stopPropagation()}>
      <Input
        type="text"
        variant="filled"
        inputSize="small"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
        prefix={node.isDirectory ? <FolderOpen size={14} /> : <FileText size={14} />}
        autoFocus
      />
    </div>
  );
};

export const FileTreeNode: React.FC<ExtendedFileTreeNodeProps> = ({
  node,
  level,
  isSelected = false,
  isExpanded = false,
  selectedFile,
  expandedFolders,
  onSelect,
  onToggleExpand,
  className = '',
  workspacePath,
  renamingPath,
  onRename,
  onCancelRename,
  renderContent,
  renderActions
}) => {
  const { t } = useI18n('tools');
  const indentDepth = getPathDepth(node.path, workspacePath);
  const handleClick = (e: React.MouseEvent) => {
    if (e.button !== 0) {
      return;
    }
    
    e.stopPropagation();
    
    const target = e.currentTarget as HTMLElement;
    if (target && typeof target.focus === 'function') {
      target.focus();
    }
    
    if (node.isDirectory && onToggleExpand) {
      onToggleExpand(node.path);
    }
    if (onSelect) {
      onSelect(node);
    }
  };

  const handleExpandClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (onToggleExpand) {
      onToggleExpand(node.path);
    }
  };
  
  const dragImageRef = React.useRef<HTMLDivElement | null>(null);
  
  const handleDragStart = (e: React.DragEvent) => {
    const dragImage = document.createElement('div');
    dragImage.textContent = t('fileTree.draggingFile', { name: node.name });
    dragImage.style.position = 'absolute';
    dragImage.style.top = '-1000px';
    dragImage.style.padding = '8px';
    dragImage.style.background = 'rgba(0, 0, 0, 0.8)';
    dragImage.style.color = 'white';
    dragImage.style.borderRadius = '4px';
    document.body.appendChild(dragImage);
    dragImageRef.current = dragImage;
    
    e.dataTransfer.setDragImage(dragImage, 0, 0);
    
    // Do not remove the drag image until dragend; removing it earlier can cancel the drag.
    
    e.dataTransfer.effectAllowed = 'copy';
    
    const payload = fileTreeDragSource.createPayload(node);
    dragManager.startDrag(fileTreeDragSource, payload, e.nativeEvent);
  };
  
  const handleDragEnd = (e: React.DragEvent) => {
    if (dragImageRef.current && document.body.contains(dragImageRef.current)) {
      document.body.removeChild(dragImageRef.current);
      dragImageRef.current = null;
    }
    
    const success = e.nativeEvent.dataTransfer?.dropEffect !== 'none';
    dragManager.endDrag(e.nativeEvent, success);
  };

  const isCompressed = node.isCompressed;
  const tooltip = isCompressed ? getCompressionTooltip(node as any) : node.path;

  const isRenaming = renamingPath === node.path;

  const handleContextMenu = (_e: React.MouseEvent) => {
  };

  const getGitStatusClass = () => {
    if (!node.isDirectory && node.gitStatus) {
      return `bitfun-file-explorer__node--git-${node.gitStatus}`;
    }
    
    if (node.isDirectory && !isExpanded && node.hasChildrenGitChanges) {
      const dirStatus = getDirectoryGitStatus(node.childrenGitStatuses);
      if (dirStatus) {
        return `bitfun-file-explorer__node--git-${dirStatus}`;
      }
    }
    
    return '';
  };

  return (
    <div className={`bitfun-file-explorer__node ${className} ${getGitStatusClass()}`}>
      <div 
        className={`bitfun-file-explorer__node-content ${isSelected ? 'bitfun-file-explorer__node-content--selected' : ''} ${node.isDirectory ? 'bitfun-file-explorer__node-content--directory' : ''} ${isCompressed ? 'bitfun-file-explorer__node-content--compressed' : ''}`}
        style={{ paddingLeft: `${(indentDepth - 1) * 20 + 16}px` }}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        title={tooltip}
        draggable={true}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        data-file-path={node.path}
        data-file={!node.isDirectory}
        data-is-directory={node.isDirectory}
        data-is-expanded={node.isDirectory ? isExpanded : undefined}
        tabIndex={0}
        role="treeitem"
        aria-selected={isSelected}
      >
        {node.isDirectory ? (
          <span className={`bitfun-file-explorer__expand-icon ${isExpanded ? 'bitfun-file-explorer__expand-icon--expanded' : ''}`} onClick={handleExpandClick}>
            {isExpanded ? (
              <ChevronDown size={16} />
            ) : (
              <ChevronRight size={16} />
            )}
          </span>
        ) : (
          <span className={getFileIconClass(node, isExpanded)}>
            {getFileIcon(node, isExpanded)}
          </span>
        )}
        
        {isRenaming ? (
          <RenameInput
            node={node}
            onRename={(newName) => onRename?.(node.path, newName)}
            onCancel={onCancelRename}
          />
        ) : renderContent ? (
          renderContent(node, level)
        ) : (
          <>
            <span className={`bitfun-file-explorer__node-name ${isCompressed ? 'bitfun-file-explorer__compressed-path' : ''}`}>
              {node.name}
            </span>
            {(() => {
              if (!node.isDirectory && node.gitStatus) {
                return <GitStatusIndicator status={node.gitStatus} compact={true} />;
              }
              
              if (node.isDirectory && !isExpanded && node.hasChildrenGitChanges) {
                const dirStatus = getDirectoryGitStatus(node.childrenGitStatuses);
                if (dirStatus) {
                  return <GitStatusIndicator status={dirStatus} compact={true} />;
                }
              }
              
              return null;
            })()}
          </>
        )}

        {renderActions && (
          <div className="bitfun-file-explorer__node-actions" onClick={(e) => e.stopPropagation()}>
            {renderActions(node)}
          </div>
        )}
      </div>

      {node.isDirectory && isExpanded && node.children && node.children.length > 0 && (
        <div className="bitfun-file-explorer__node-children">
          {node.children.map(child => (
            <FileTreeNode
              key={child.path}
              node={child}
              level={level + 1}
              isSelected={selectedFile === child.path}
              isExpanded={expandedFolders?.has(child.path) || false}
              selectedFile={selectedFile}
              expandedFolders={expandedFolders}
              onSelect={onSelect}
              onToggleExpand={onToggleExpand}
              workspacePath={workspacePath}
              renamingPath={renamingPath}
              onRename={onRename}
              onCancelRename={onCancelRename}
              renderContent={renderContent}
              renderActions={renderActions}
            />
          ))}
        </div>
      )}
    </div>
  );
};

export default FileTreeNode;
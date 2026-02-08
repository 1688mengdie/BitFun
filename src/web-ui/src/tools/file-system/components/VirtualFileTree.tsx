import React, { useCallback, useMemo, useRef, forwardRef } from 'react';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { ChevronRight, ChevronDown, Loader2 } from 'lucide-react';
import { VirtualFileTreeProps, FlatFileNode, FileSystemNode } from '../types';
import { getFileIcon, getFileIconClass } from '../utils/fileIcons';
import { GitStatusIndicator } from './GitStatusIndicator';
import { useI18n } from '@/infrastructure/i18n';

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

interface VirtualFileRowProps {
  node: FlatFileNode;
  isSelected: boolean;
  isExpanded: boolean;
  onSelect: (node: FlatFileNode) => void;
  onToggleExpand: (path: string) => void;
  workspacePath?: string;
  renamingPath?: string | null;
  onRename?: (oldPath: string, newName: string) => void;
  onCancelRename?: () => void;
}

const VirtualFileRow = React.memo<VirtualFileRowProps>(({
  node,
  isSelected,
  isExpanded,
  onSelect,
  onToggleExpand,
  // These props are reserved for future use (renaming feature)
  // workspacePath,
  // renamingPath,
  // onRename,
  // onCancelRename,
}) => {
  const indentPx = node.depth * 20 + 16;

  const handleClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    
    if (node.isDirectory) {
      onToggleExpand(node.path);
    }
    onSelect(node);
  }, [node, onSelect, onToggleExpand]);

  const handleExpandClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onToggleExpand(node.path);
  }, [node.path, onToggleExpand]);

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

  const nodeForIcon: FileSystemNode = useMemo(() => ({
    path: node.path,
    name: node.name,
    isDirectory: node.isDirectory,
    extension: node.extension,
    size: node.size,
    lastModified: node.lastModified,
    gitStatus: node.gitStatus,
    isCompressed: node.isCompressed,
  }), [node]);

  return (
    <div className={`bitfun-file-explorer__node ${getGitStatusClass()}`}>
      <div 
        className={`bitfun-file-explorer__node-content ${isSelected ? 'bitfun-file-explorer__node-content--selected' : ''} ${node.isDirectory ? 'bitfun-file-explorer__node-content--directory' : ''} ${node.isCompressed ? 'bitfun-file-explorer__node-content--compressed' : ''}`}
        style={{ paddingLeft: `${indentPx}px` }}
        onClick={handleClick}
        data-file-path={node.path}
        data-file={!node.isDirectory}
        data-is-directory={node.isDirectory}
        data-is-expanded={node.isDirectory ? isExpanded : undefined}
        tabIndex={0}
        role="treeitem"
        aria-selected={isSelected}
        title={node.path}
      >
        {node.isDirectory ? (
          <span 
            className={`bitfun-file-explorer__expand-icon ${isExpanded ? 'bitfun-file-explorer__expand-icon--expanded' : ''}`}
            onClick={handleExpandClick}
          >
            {node.isLoading ? (
              <Loader2 size={16} className="bitfun-file-explorer__loading-icon" />
            ) : isExpanded ? (
              <ChevronDown size={16} />
            ) : (
              <ChevronRight size={16} />
            )}
          </span>
        ) : (
          <span className={getFileIconClass(nodeForIcon, false)}>
            {getFileIcon(nodeForIcon, false)}
          </span>
        )}
        
        <span className={`bitfun-file-explorer__node-name ${node.isCompressed ? 'bitfun-file-explorer__compressed-path' : ''}`}>
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
      </div>
    </div>
  );
});

VirtualFileRow.displayName = 'VirtualFileRow';

export const VirtualFileTree = forwardRef<VirtuosoHandle, VirtualFileTreeProps>(({
  flatNodes,
  selectedFile,
  expandedFolders,
  onNodeSelect,
  onToggleExpand,
  height = '100%',
  className = '',
  workspacePath,
  renamingPath,
  onRename,
  onCancelRename,
}, ref) => {
  const { t } = useI18n('tools');
  const virtuosoRef = useRef<VirtuosoHandle>(null);

  React.useImperativeHandle(ref, () => virtuosoRef.current!, []);

  const handleNodeSelect = useCallback((node: FlatFileNode) => {
    onNodeSelect?.(node);
  }, [onNodeSelect]);

  const handleToggleExpand = useCallback((path: string) => {
    onToggleExpand?.(path);
  }, [onToggleExpand]);

  const itemContent = useCallback((_index: number, node: FlatFileNode) => {
    const isSelected = selectedFile === node.path;
    const isExpanded = expandedFolders.has(node.path);

    return (
      <VirtualFileRow
        node={node}
        isSelected={isSelected}
        isExpanded={isExpanded}
        onSelect={handleNodeSelect}
        onToggleExpand={handleToggleExpand}
        workspacePath={workspacePath}
        renamingPath={renamingPath}
        onRename={onRename}
        onCancelRename={onCancelRename}
      />
    );
  }, [selectedFile, expandedFolders, handleNodeSelect, handleToggleExpand, workspacePath, renamingPath, onRename, onCancelRename]);

  if (flatNodes.length === 0) {
    return (
      <div className={`bitfun-file-explorer__tree bitfun-file-explorer__tree--empty ${className}`}>
        <div className="bitfun-file-explorer__empty-message">
          <p>{t('fileTree.empty')}</p>
        </div>
      </div>
    );
  }

  return (
    <div 
      className={`bitfun-file-explorer__tree bitfun-file-explorer__tree--virtual ${className}`}
      style={{ height }}
      tabIndex={0}
    >
      <Virtuoso
        ref={virtuosoRef}
        data={flatNodes}
        itemContent={itemContent}
        overscan={50}
        increaseViewportBy={{ top: 100, bottom: 200 }}
        style={{ height: '100%' }}
        computeItemKey={(_index, node) => node.path}
      />
    </div>
  );
});

VirtualFileTree.displayName = 'VirtualFileTree';

export default VirtualFileTree;

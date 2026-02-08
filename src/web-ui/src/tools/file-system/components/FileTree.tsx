import React, { useState, useCallback, useMemo, useEffect } from 'react';
import { FileTreeNode } from './FileTreeNode';
import { FileTreeProps, FileSystemNode } from '../types';
import { lazyCompressFileTree, shouldCompressPaths, CompressedNode } from '../utils/pathCompression';
import { Input } from '@/component-library';
import { FileText, FolderOpen } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';

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
        // Select filename without extension.
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

export const FileTree: React.FC<FileTreeProps> = ({
  nodes,
  selectedFile,
  expandedFolders: externalExpandedFolders,
  onNodeSelect,
  onNodeExpand,
  className = '',
  level = 0,
  workspacePath,
  renderNodeContent,
  renderNodeActions,
  renamingPath,
  onRename,
  onCancelRename
}) => {
  const { t } = useI18n('tools');
  const [internalExpandedFolders, setInternalExpandedFolders] = useState<Set<string>>(new Set());
  
  const expandedFolders = externalExpandedFolders || internalExpandedFolders;

  const handleNodeExpand = useCallback((path: string) => {
    if (onNodeExpand) {
      const isCurrentlyExpanded = expandedFolders.has(path);
      onNodeExpand(path, !isCurrentlyExpanded);
    } else {
      setInternalExpandedFolders(prev => {
        const newSet = new Set(prev);
        if (newSet.has(path)) {
          newSet.delete(path);
        } else {
          newSet.add(path);
        }
        return newSet;
      });
    }
  }, [expandedFolders, onNodeExpand]);

  const processedNodes = useMemo(() => {
    if (!shouldCompressPaths()) {
      return nodes;
    }
    return lazyCompressFileTree(nodes, expandedFolders);
  }, [nodes, expandedFolders]);

  const renderNodes = (nodeList: CompressedNode[], currentLevel: number = level) => {
    return nodeList.map(node => (
      <FileTreeNode
        key={node.path}
        node={node}
        level={currentLevel}
        isSelected={selectedFile === node.path}
        isExpanded={expandedFolders.has(node.path)}
        selectedFile={selectedFile}
        expandedFolders={expandedFolders}
        onSelect={onNodeSelect}
        onToggleExpand={handleNodeExpand}
        renamingPath={renamingPath}
        onRename={onRename}
        onCancelRename={onCancelRename}
        renderContent={renderNodeContent}
        renderActions={renderNodeActions}
        workspacePath={workspacePath}
      />
    ));
  };

  return (
    <div 
      className={`bitfun-file-explorer__tree ${className}`}
      tabIndex={0}
    >
      {processedNodes.length > 0 ? (
        renderNodes(processedNodes)
      ) : (
        <div className="bitfun-file-explorer__empty-message">
          <p>{t('fileTree.empty')}</p>
        </div>
      )}
    </div>
  );
};

export default FileTree;
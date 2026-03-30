export { FileExplorer, FileTree, FileTreeNode } from './components';

export { useFileSystem, useFileTree } from './hooks';
export { useFileTreeGitSync } from './hooks/useFileTreeGitSync';

export type {
  FileSystemNode,
  FileExplorerProps,
  FileExplorerToolbarHandlers,
  FileTreeProps,
  FileTreeNodeProps,
  FileSystemOptions,
  FileSystemState
} from './types';

export { getNewItemParentPath } from './utils/getNewItemParentPath';

export {
  getFileIcon,
  getFileIconClass,
  isImageFile,
  isCodeFile,
  isConfigFile
} from './utils/fileIcons';

export {
  compressFileTree,
  lazyCompressFileTree,
  shouldCompressPaths,
  getCompressionTooltip
} from './utils/pathCompression';

export { fileSystemService } from './services/FileSystemService';
/**
 * Unified exports for ContentCanvas types.
 */

// Tab-related types
export type {
  TabState,
  CanvasTab,
  EditorGroupState,
  TabDragPayload,
  TabActions,
  TabVisualProps,
  ClosedTabRecord,
} from './tab';

export { generateTabId, createTab } from './tab';

// Layout-related types
export type {
  SplitMode,
  AnchorPosition,
  DropPosition,
  EditorGroupId,
  LayoutState,
  CanvasState,
  CanvasPersistState,
  Grid9Slot,
  Grid9Template,
} from './layout';

export {
  LAYOUT_CONFIG,
  GRID_MAX_DIM,
  GRID9_RATIO_CONFIG,
  EDITOR_GROUP_IDS,
  EDITOR_GROUP_COL,
  EDITOR_GROUP_ROW,
  createEditorGroupState,
  createLayoutState,
  createCanvasState,
  clampSplitRatio,
  clampAnchorSize,
  clampGrid9Ratio,
} from './layout';

// Content-related types
export type {
  PanelContentType,
  PanelContent,
  CreateTabOptions,
  CreateTabEventDetail,
} from './content';

export {
  FILE_VIEWER_TYPES,
  isFileViewerType,
  TAB_EVENTS,
} from './content';

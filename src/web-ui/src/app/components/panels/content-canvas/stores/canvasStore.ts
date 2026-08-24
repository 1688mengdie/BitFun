/**
 * Canvas Store - canvas state management.
 * Uses Zustand to manage tabs and layout state.
 */

import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { createContext, useContext } from 'react';
import type {
  CanvasTab,
  EditorGroupId,
  EditorGroupState,
  LayoutState,
  TabState,
  PanelContent,
  ClosedTabRecord,
  SplitMode,
  AnchorPosition,
  DropPosition,
} from '../types';
import {
  createTab,
  createEditorGroupState,
  createLayoutState,
  clampSplitRatio,
  clampAnchorSize,
  clampGrid9Ratio,
  GRID_MAX_DIM,
  EDITOR_GROUP_IDS,
  EDITOR_GROUP_COL,
  EDITOR_GROUP_ROW,
} from '../types';
import { normalizePath } from '@/shared/utils/pathUtils';

// ==================== Store State Types ====================

interface CanvasStoreState {
  primaryGroup: EditorGroupState;
  secondaryGroup: EditorGroupState;
  tertiaryGroup: EditorGroupState;
  activeGroupId: EditorGroupId;
  layout: LayoutState;
  isMissionControlOpen: boolean;
  draggingTabId: string | null;
  draggingFromGroupId: EditorGroupId | null;
  closedTabs: ClosedTabRecord[];
  maxClosedTabsHistory: number;
}

interface CanvasStoreActions {
  // ==================== Tab Operations ====================
  
  /** Add tab */
  addTab: (content: PanelContent, state?: TabState, groupId?: EditorGroupId) => void;
  
  /** Close tab; forceRemove removes terminal tab instead of hiding */
  closeTab: (tabId: string, groupId: EditorGroupId, options?: { forceRemove?: boolean }) => void;

  /** Close and remove tab by terminal sessionId (sync when left panel closes terminal) */
  closeTerminalTabBySessionId: (sessionId: string) => void;

  /** Rename terminal tab by sessionId (sync when left panel renames terminal) */
  renameTerminalTabBySessionId: (sessionId: string, newName: string) => void;
  
  /** Close all tabs */
  closeAllTabs: (groupId?: EditorGroupId) => void;
  
  /** Switch to tab */
  switchToTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Update tab content */
  updateTabContent: (tabId: string, groupId: EditorGroupId, content: PanelContent) => void;
  
  /** Set tab dirty state */
  setTabDirty: (tabId: string, groupId: EditorGroupId, isDirty: boolean) => void;

  /** Mark whether the tab's file is missing on disk (editor-detected) */
  setTabFileDeletedFromDisk: (tabId: string, groupId: EditorGroupId, deleted: boolean) => void;
  
  /** Promote tab state (preview -> active) */
  promoteTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Pin/unpin tab */
  togglePinTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Find tab by metadata */
  findTabByMetadata: (metadata: Record<string, any>) => { tab: CanvasTab; groupId: EditorGroupId } | null;
  
  /** Reopen recently closed tab */
  reopenClosedTab: () => void;
  
  /** Hide tab (keep state) */
  hideTab: (tabId: string, groupId: EditorGroupId) => void;
  
  /** Show hidden tab */
  showTab: (tabId: string, groupId: EditorGroupId) => void;
  
  // ==================== Drag Operations ====================
  
  /** Start drag */
  startDrag: (tabId: string, groupId: EditorGroupId) => void;
  
  /** End drag */
  endDrag: () => void;
  
  /** Move tab to another group */
  moveTabToGroup: (tabId: string, fromGroupId: EditorGroupId, toGroupId: EditorGroupId, index?: number) => void;
  
  /** Reorder tabs */
  reorderTab: (tabId: string, groupId: EditorGroupId, newIndex: number) => void;
  
  /** Handle drop */
  handleDrop: (tabId: string, fromGroupId: EditorGroupId, toGroupId: EditorGroupId, position?: DropPosition) => void;
  
  // ==================== Layout Operations ====================
  
  /** Set split mode */
  setSplitMode: (mode: SplitMode) => void;
  
  /** Set split ratio */
  setSplitRatio: (ratio: number) => void;

  /** Set secondary split ratio used by grid top row */
  setSplitRatio2: (ratio: number) => void;
  
  /** Set anchor position */
  setAnchorPosition: (position: AnchorPosition) => void;
  
  /** Set anchor size */
  setAnchorSize: (size: number) => void;
  
  /** Toggle maximize */
  toggleMaximize: () => void;
  
  /** Set active editor group */
  setActiveGroup: (groupId: EditorGroupId) => void;
  
  // ==================== Mission Control ====================
  
  /** Open mission control */
  openMissionControl: () => void;
  
  /** Close mission control */
  closeMissionControl: () => void;
  
  /** Toggle mission control */
  toggleMissionControl: () => void;
  
  // ==================== State Management ====================
  
  /** Reset state */
  reset: () => void;
  
  /** Get all tabs */
  getAllTabs: () => CanvasTab[];

  // ==================== Grid9 operations ====================

  /** Enter grid9 mode, seeding grid9Cells from existing content and clamping counts */
  enterGrid9: (cols: number, rows: number) => void;

  /** Apply a preset grid9 template (rows×cols), resetting ratios and moving out-of-template tabs into primary */
  applyGrid9Template: (cols: number, rows: number) => void;

  /** Merge two grid9 cells: all tabs from `fromGroupId` move into `toGroupId`, source is emptied */
  mergeGrid9Cells: (fromGroupId: EditorGroupId, toGroupId: EditorGroupId) => void;

  /** Remove a blank grid9 cell: shrink the grid by one column/row and shift remaining cells */
  removeGrid9Cell: (groupId: EditorGroupId) => void;

  /** Set a grid9 column ratio, renormalising the axis to sum to 1 */
  setGrid9ColRatio: (col: number, ratio: number) => void;

  /** Set a grid9 row ratio, renormalising the axis to sum to 1 */
  setGrid9RowRatio: (row: number, ratio: number) => void;

  /** All groups (legacy + grid9 cells) that currently have renderable tabs */
  getAllRenderableGroups: () => { id: EditorGroupId; group: EditorGroupState }[];
}

type CanvasStore = CanvasStoreState & CanvasStoreActions;

// ==================== Initial State ====================

const initialState: CanvasStoreState = {
  primaryGroup: createEditorGroupState(),
  secondaryGroup: createEditorGroupState(),
  tertiaryGroup: createEditorGroupState(),
  activeGroupId: 'primary',
  layout: createLayoutState(),
  isMissionControlOpen: false,
  draggingTabId: null,
  draggingFromGroupId: null,
  closedTabs: [],
  maxClosedTabsHistory: 10,
};

const getGroup = (draft: CanvasStoreState, groupId: EditorGroupId): EditorGroupState => {
  // grid9 mode: the single grid9Cells map is the source of truth; the three
  // legacy fields are dormant. Reading a slot that has no cell yet materialises
  // an empty cell so callers can mutate it in place (write-through).
  if (draft.layout.splitMode === 'grid9') {
    let cell = draft.layout.grid9Cells[groupId];
    if (!cell) {
      cell = createEditorGroupState();
      draft.layout.grid9Cells[groupId] = cell;
    }
    return cell;
  }
  if (groupId === 'primary') return draft.primaryGroup;
  if (groupId === 'secondary') return draft.secondaryGroup;
  return draft.tertiaryGroup;
};

const getVisibleTabs = (group: EditorGroupState) => group.tabs.filter(t => !t.isHidden);
const getVisibleCount = (group: EditorGroupState) => getVisibleTabs(group).length;

const ensureValidActiveTab = (group: EditorGroupState) => {
  const visibleTabs = getVisibleTabs(group);
  if (visibleTabs.length === 0) {
    group.activeTabId = null;
  } else if (group.activeTabId === null || !visibleTabs.find(t => t.id === group.activeTabId)) {
    group.activeTabId = visibleTabs[0]?.id || null;
  }
};

// ==================== Grid9 helpers ====================

/** Clamp a grid9 dimension (columns/rows) to 1..GRID_MAX_DIM. */
const clampGrid9Dim = (n: number): number => Math.min(GRID_MAX_DIM, Math.max(1, Math.round(n)));

/**
 * Move content from the three legacy groups into the grid9Cells map when
 * entering grid9 mode, then clear the legacy groups (which stay dormant while
 * grid9 is active). Content is keyed by its canonical slot id.
 */
const seedGrid9CellsFromLegacy = (draft: CanvasStoreState) => {
  const moveCell = (gid: EditorGroupId, legacy: EditorGroupState) => {
    if (legacy.tabs.length === 0) return;
    const cell = draft.layout.grid9Cells[gid] ?? createEditorGroupState();
    cell.tabs = [...cell.tabs, ...legacy.tabs];
    if (legacy.activeTabId && cell.tabs.some(t => t.id === legacy.activeTabId)) {
      cell.activeTabId = legacy.activeTabId;
    } else if (!cell.activeTabId && cell.tabs.length > 0) {
      cell.activeTabId = cell.tabs[0].id;
    }
    draft.layout.grid9Cells[gid] = cell;
  };
  moveCell('primary', draft.primaryGroup);
  moveCell('secondary', draft.secondaryGroup);
  moveCell('tertiary', draft.tertiaryGroup);
  draft.primaryGroup = createEditorGroupState();
  draft.secondaryGroup = createEditorGroupState();
  draft.tertiaryGroup = createEditorGroupState();
};

/** Reset grid9 ratios to equal shares for a cols×rows template (sum === 1). */
const resetGrid9Ratios = (layout: LayoutState, cols: number, rows: number) => {
  layout.grid9ColRatios = Array.from({ length: cols }, () => 1 / cols);
  layout.grid9RowRatios = Array.from({ length: rows }, () => 1 / rows);
};

/**
 * Grow a grid9 ratio array to `targetLen` entries. The new last share gets an
 * equal share of the axis; the existing shares are scaled proportionally so
 * their relative proportions are preserved and the axis still sums to 1.
 */
const growGrid9RatiosTo = (ratios: number[], targetLen: number): number[] => {
  let result = [...ratios];
  while (result.length < targetLen) {
    const weight = 1 / (result.length + 1);
    const scale = 1 - weight;
    result = result.map(r => r * scale);
    result.push(weight);
  }
  return result;
};

/** Shrink a grid9 ratio array to `targetLen` by keeping the first entries and renormalising the sum to 1. */
const shrinkGrid9Ratios = (ratios: number[], targetLen: number): number[] => {
  if (targetLen <= 0) return [1];
  const kept = ratios.slice(0, targetLen);
  const sum = kept.reduce((a, b) => a + b, 0);
  if (sum === 0) return Array.from({ length: targetLen }, () => 1 / targetLen);
  return kept.map(r => r / sum);
};

/** Remove a single ratio at `idx` and renormalise the remaining entries to sum to 1. */
const removeGrid9RatioAt = (ratios: number[], idx: number): number[] => {
  const kept = ratios.filter((_, i) => i !== idx);
  if (kept.length === 0) return [1];
  const sum = kept.reduce((a, b) => a + b, 0);
  if (sum === 0) return Array.from({ length: kept.length }, () => 1 / kept.length);
  return kept.map(r => r / sum);
};

const cellHasVisibleTabs = (layout: LayoutState, gid: EditorGroupId): boolean => {
  const cell = layout.grid9Cells[gid];
  return !!cell && cell.tabs.some(t => !t.isHidden);
};

const trailingRowHasTabs = (layout: LayoutState, row: number, colsCount: number): boolean => {
  for (let c = 0; c < colsCount; c++) {
    if (cellHasVisibleTabs(layout, EDITOR_GROUP_IDS[row * GRID_MAX_DIM + c])) return true;
  }
  return false;
};

const trailingColHasTabs = (layout: LayoutState, col: number, rowsCount: number): boolean => {
  for (let r = 0; r < rowsCount; r++) {
    if (cellHasVisibleTabs(layout, EDITOR_GROUP_IDS[r * GRID_MAX_DIM + col])) return true;
  }
  return false;
};

/**
 * Shrink trailing empty rows (then trailing empty columns) of a grid9 layout,
 * each down to a minimum of 1, renormalising the corresponding ratio array so
 * the axis still sums to 1. Shared by closeTab / closeAllTabs / handleDrop /
 * removeGrid9Cell. Rows are collapsed first so a row removed by column collapse
 * cannot leave a phantom empty column.
 */
const collapseTrailingGrid9 = (layout: LayoutState) => {
  let rows = layout.grid9RowsCount;
  while (rows > 1 && !trailingRowHasTabs(layout, rows - 1, layout.grid9ColsCount)) {
    rows -= 1;
  }
  let cols = layout.grid9ColsCount;
  while (cols > 1 && !trailingColHasTabs(layout, cols - 1, rows)) {
    cols -= 1;
  }
  if (rows !== layout.grid9RowsCount) {
    layout.grid9RowsCount = rows;
    layout.grid9RowRatios = shrinkGrid9Ratios(layout.grid9RowRatios, rows);
  }
  if (cols !== layout.grid9ColsCount) {
    layout.grid9ColsCount = cols;
    layout.grid9ColRatios = shrinkGrid9Ratios(layout.grid9ColRatios, cols);
  }
};

/**
 * Keep activeGroupId pointing at a live, in-template grid9 cell; fall back to
 * the first non-empty in-template cell, then to primary.
 */
const fixGrid9Active = (draft: CanvasStoreState) => {
  const layout = draft.layout;
  const activeCell = layout.grid9Cells[draft.activeGroupId];
  const activeRow = EDITOR_GROUP_ROW[draft.activeGroupId];
  const activeCol = EDITOR_GROUP_COL[draft.activeGroupId];
  const activeEmpty = !activeCell || getVisibleCount(activeCell) === 0;
  if (activeRow >= layout.grid9RowsCount || activeCol >= layout.grid9ColsCount || activeEmpty) {
    const firstNonEmpty = EDITOR_GROUP_IDS.find(gid => {
      const cell = layout.grid9Cells[gid];
      return (
        EDITOR_GROUP_ROW[gid] < layout.grid9RowsCount &&
        EDITOR_GROUP_COL[gid] < layout.grid9ColsCount &&
        !!cell &&
        getVisibleCount(cell) > 0
      );
    });
    draft.activeGroupId = firstNonEmpty ?? 'primary';
  }
};

/**
 * grid9 close-tab lifecycle: remove the tab from its cell (or hide a terminal),
 * collapse trailing empty rows/columns and fix the active slot. Never enters the
 * legacy three-field degradation matrix.
 */
const grid9CloseCellTab = (
  draft: CanvasStoreState,
  groupId: EditorGroupId,
  tabId: string,
  options?: { forceRemove?: boolean },
) => {
  const group = getGroup(draft, groupId);
  const tabIndex = group.tabs.findIndex(t => t.id === tabId);
  if (tabIndex === -1) return;

  const tab = group.tabs[tabIndex];
  const forceRemove = options?.forceRemove === true;

  // Terminal tabs without force remove: hide instead of deleting for reactivation.
  if (tab.content.type === 'terminal' && !forceRemove) {
    tab.isHidden = true;
    if (group.activeTabId === tabId) {
      const visibleTabs = group.tabs.filter(t => !t.isHidden);
      group.activeTabId = visibleTabs[0]?.id || null;
    }
    return;
  }

  if (!(tab.content.type === 'terminal' && forceRemove)) {
    draft.closedTabs.unshift({ tab: { ...tab }, closedAt: Date.now(), groupId, index: tabIndex });
    if (draft.closedTabs.length > draft.maxClosedTabsHistory) {
      draft.closedTabs.pop();
    }
  }

  group.tabs.splice(tabIndex, 1);
  ensureValidActiveTab(group);
  collapseTrailingGrid9(draft.layout);
  fixGrid9Active(draft);
};

/**
 * grid9 close-all lifecycle: keep pinned tabs in each cell, collect surviving
 * pinned tabs into the primary cell, clear the rest and collapse to a single
 * column. Must never drop a pinned tab from any slot.
 */
const grid9CloseAllCells = (draft: CanvasStoreState) => {
  const layout = draft.layout;
  const collected: CanvasTab[] = [];
  for (const gid of EDITOR_GROUP_IDS) {
    const cell = layout.grid9Cells[gid];
    if (!cell) continue;
    const pinned = cell.tabs.filter(t => t.state === 'pinned');
    if (pinned.length > 0) {
      collected.push(...pinned);
    }
    delete layout.grid9Cells[gid];
  }
  // Restore surviving pinned tabs into the legacy primary group (we drop to a
  // single column), so none of them are lost and the canvas still renders them.
  draft.primaryGroup =
    collected.length > 0
      ? { tabs: collected, activeTabId: collected[0]?.id ?? null }
      : createEditorGroupState();
  draft.secondaryGroup = createEditorGroupState();
  draft.tertiaryGroup = createEditorGroupState();
  draft.activeGroupId = 'primary';
  draft.layout.splitMode = 'none';
  layout.grid9Cells = {};
  layout.grid9ColsCount = 1;
  layout.grid9RowsCount = 1;
  layout.grid9ColRatios = [1];
  layout.grid9RowRatios = [1];
};

const keepPinnedTabsOnly = (group: EditorGroupState) => {
  group.tabs = group.tabs.filter(tab => tab.state === 'pinned');
  ensureValidActiveTab(group);
};

const getPinnedBoundary = (group: EditorGroupState) => {
  const firstUnpinnedIndex = group.tabs.findIndex(tab => tab.state !== 'pinned');
  return firstUnpinnedIndex === -1 ? group.tabs.length : firstUnpinnedIndex;
};

const insertTabRespectingPinnedBoundary = (group: EditorGroupState, tab: CanvasTab) => {
  const insertIndex = getPinnedBoundary(group);
  group.tabs.splice(insertIndex, 0, tab);
};

// ==================== Store Creation ====================

const createCanvasStoreHook = () => create<CanvasStore>()(
  immer((set, get) => ({
      ...initialState,
      
      // ==================== Tab Operations ====================
      
      addTab: (content, state = 'preview', groupId) => {
        set((draft) => {
          let targetGroupId = groupId || draft.activeGroupId;
          
          // Adjust target group based on splitMode to ensure visibility
          if (draft.layout.splitMode === 'none') {
            // Single-column mode: use primary group only
            targetGroupId = 'primary';
            draft.activeGroupId = 'primary';
          } else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
            // Two-column mode: use primary or secondary (not tertiary)
            if (targetGroupId === 'tertiary') {
              targetGroupId = draft.activeGroupId === 'primary' ? 'primary' : 'secondary';
              draft.activeGroupId = targetGroupId;
            }
          }
          // Grid mode: all three groups are allowed
          
          const group = getGroup(draft, targetGroupId);
          
          if (state === 'preview') {
            const previewIndex = group.tabs.findIndex(
              t => t.state === 'preview' && !t.isHidden
            );
            if (previewIndex !== -1) {
              group.tabs.splice(previewIndex, 1);
            }
          }
          
          const newTab = createTab(content, state);
          insertTabRespectingPinnedBoundary(group, newTab);
          group.activeTabId = newTab.id;
          draft.activeGroupId = targetGroupId;
        });
      },
      
      closeTab: (tabId, groupId, options) => {
        set((draft) => {
          // grid9: run the dedicated lifecycle and never enter the legacy
          // three-field degradation matrix.
          if (draft.layout.splitMode === 'grid9') {
            grid9CloseCellTab(draft, groupId, tabId, options);
            return;
          }

          const group = getGroup(draft, groupId);
          const tabIndex = group.tabs.findIndex(t => t.id === tabId);
          
          if (tabIndex === -1) return;
          
          const tab = group.tabs[tabIndex];
          const forceRemove = options?.forceRemove === true;

          // For terminal tabs without force remove, hide instead of delete for reactivation
          if (tab.content.type === 'terminal' && !forceRemove) {
            tab.isHidden = true;
            
            // If closing active tab, switch to next visible tab
            if (group.activeTabId === tabId) {
              const visibleTabs = group.tabs.filter(t => !t.isHidden);
              group.activeTabId = visibleTabs[0]?.id || null;
            }
            return;
          }
          
          // Skip history when terminal is force-removed
          if (!(tab.content.type === 'terminal' && forceRemove)) {
            // Record in close history
            draft.closedTabs.unshift({
              tab: { ...tab },
              closedAt: Date.now(),
              groupId,
              index: tabIndex,
            });
            // Limit history size
            if (draft.closedTabs.length > draft.maxClosedTabsHistory) {
              draft.closedTabs.pop();
            }
          }
          
          // Remove tab
          group.tabs.splice(tabIndex, 1);
          
          // If closing active tab, switch to adjacent tab
          if (group.activeTabId === tabId) {
            const visibleTabs = group.tabs.filter(t => !t.isHidden);
            if (visibleTabs.length > 0) {
              const nextIndex = Math.min(tabIndex, visibleTabs.length - 1);
              group.activeTabId = visibleTabs[nextIndex]?.id || null;
            } else {
              group.activeTabId = null;
            }
          }
          
          // Auto-merge empty editor groups
          const getVisibleCount = (g: EditorGroupState) => g.tabs.filter(t => !t.isHidden).length;
          const getVisibleTabs = (g: EditorGroupState) => g.tabs.filter(t => !t.isHidden);
          
          const pCount = getVisibleCount(draft.primaryGroup);
          const sCount = getVisibleCount(draft.secondaryGroup);
          const tCount = getVisibleCount(draft.tertiaryGroup);
          
          // Helper: ensure activeTabId is valid
          const ensureValidActiveTab = (group: EditorGroupState) => {
            const visibleTabs = getVisibleTabs(group);
            if (visibleTabs.length === 0) {
              group.activeTabId = null;
            } else if (group.activeTabId === null || !visibleTabs.find(t => t.id === group.activeTabId)) {
              // If activeTabId is invalid, use first visible tab
              group.activeTabId = visibleTabs[0]?.id || null;
            }
          };
          
          // Helper: merge tabs from multiple groups into primary
          const mergeGroupsToPrimary = (sourceGroups: EditorGroupId[]) => {
            const allTabs: CanvasTab[] = [];
            let activeTabId: string | null = null;
            
            // Prefer active tab from current active group
            const currentActiveGroupId = draft.activeGroupId;
            if (sourceGroups.includes(currentActiveGroupId)) {
              const currentGroup = getGroup(draft, currentActiveGroupId);
              const visibleTabs = getVisibleTabs(currentGroup);
              if (currentGroup.activeTabId && visibleTabs.find(t => t.id === currentGroup.activeTabId)) {
                activeTabId = currentGroup.activeTabId;
              }
            }
            
            // Collect all visible tabs
            for (const sourceGroupId of sourceGroups) {
              const sourceGroup = getGroup(draft, sourceGroupId);
              const visibleTabs = getVisibleTabs(sourceGroup);
              allTabs.push(...visibleTabs);
              
              // If active tab not chosen, use one from source group if still visible
              if (!activeTabId && sourceGroup.activeTabId && visibleTabs.find(t => t.id === sourceGroup.activeTabId)) {
                activeTabId = sourceGroup.activeTabId;
              }
            }
            
            // Merge into primary group
            draft.primaryGroup.tabs = allTabs;
            draft.primaryGroup.activeTabId = activeTabId || (allTabs.length > 0 ? allTabs[0].id : null);
            
            // Reset other groups
            draft.secondaryGroup = createEditorGroupState();
            draft.tertiaryGroup = createEditorGroupState();
          };
          
          if (draft.layout.splitMode === 'grid') {
            if (tCount === 0 && pCount > 0 && sCount > 0) {
              // Tertiary empty; primary + secondary have tabs -> downgrade to horizontal
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'horizontal';
              if (draft.activeGroupId === 'tertiary') {
                // If tertiary was active, switch to primary (tertiary is empty)
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
              }
            } else if (tCount === 0 && (pCount === 0 || sCount === 0)) {
              // Tertiary empty and primary/secondary missing -> merge remaining to primary
              const remainingGroups: EditorGroupId[] = [];
              if (pCount > 0) remainingGroups.push('primary');
              if (sCount > 0) remainingGroups.push('secondary');
              
              if (remainingGroups.length > 0) {
                mergeGroupsToPrimary(remainingGroups);
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              } else {
                // All groups are empty
                draft.primaryGroup = createEditorGroupState();
                draft.secondaryGroup = createEditorGroupState();
                draft.tertiaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              }
            } else if (pCount === 0 && sCount === 0 && tCount > 0) {
              // Primary + secondary empty; tertiary has tabs -> merge to primary
              mergeGroupsToPrimary(['tertiary']);
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (pCount === 0 && sCount > 0) {
              // Primary empty; secondary and tertiary have tabs
              // Move secondary -> primary (top), tertiary -> secondary (bottom)
              // Because secondary (top-right) and tertiary (bottom) are vertical -> downgrade to vertical
              const sTabs = getVisibleTabs(draft.secondaryGroup);
              const tTabs = getVisibleTabs(draft.tertiaryGroup);
              
              draft.primaryGroup.tabs = sTabs;
              draft.primaryGroup.activeTabId = draft.secondaryGroup.activeTabId && 
                sTabs.find(t => t.id === draft.secondaryGroup.activeTabId) 
                  ? draft.secondaryGroup.activeTabId 
                  : (sTabs[0]?.id || null);
              
              draft.secondaryGroup.tabs = tTabs;
              draft.secondaryGroup.activeTabId = draft.tertiaryGroup.activeTabId && 
                tTabs.find(t => t.id === draft.tertiaryGroup.activeTabId) 
                  ? draft.tertiaryGroup.activeTabId 
                  : (tTabs[0]?.id || null);
              
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';
              
              // If activeGroupId points to merged group, switch appropriately
              if (draft.activeGroupId === 'secondary') {
                draft.activeGroupId = 'primary';
              } else if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
              // If activeGroupId is already 'primary', keep it
            } else if (sCount === 0 && pCount > 0) {
              // Secondary empty; primary and tertiary have tabs
              // Move tertiary -> secondary
              // Because primary (top-left) and tertiary (bottom) are vertical -> downgrade to vertical
              const tTabs = getVisibleTabs(draft.tertiaryGroup);
              draft.secondaryGroup.tabs = tTabs;
              draft.secondaryGroup.activeTabId = draft.tertiaryGroup.activeTabId && 
                tTabs.find(t => t.id === draft.tertiaryGroup.activeTabId) 
                  ? draft.tertiaryGroup.activeTabId 
                  : (tTabs[0]?.id || null);
              
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';
              
              // If activeGroupId points to tertiary, switch to secondary
              if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
            }
            
            // Ensure activeTabId is valid for all groups
            ensureValidActiveTab(draft.primaryGroup);
            ensureValidActiveTab(draft.secondaryGroup);
            ensureValidActiveTab(draft.tertiaryGroup);
          } else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
            if (sCount === 0 && pCount > 0) {
              // Secondary empty; primary has tabs -> merge to single column
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
              ensureValidActiveTab(draft.primaryGroup);
            } else if (pCount === 0 && sCount > 0) {
              // Primary empty; secondary has tabs -> merge to primary
              mergeGroupsToPrimary(['secondary']);
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (pCount === 0 && sCount === 0) {
              // Both groups are empty
              draft.primaryGroup = createEditorGroupState();
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            }
          }
          
          // Final check: ensure activeGroupId points to a group with tabs
          const finalPCount = getVisibleCount(draft.primaryGroup);
          const finalSCount = getVisibleCount(draft.secondaryGroup);
          const finalTCount = getVisibleCount(draft.tertiaryGroup);
          
          if (draft.activeGroupId === 'primary' && finalPCount === 0) {
            // Primary empty; switch to group with tabs
            if (finalSCount > 0) {
              draft.activeGroupId = 'secondary';
            } else if (finalTCount > 0) {
              draft.activeGroupId = 'tertiary';
            }
          } else if (draft.activeGroupId === 'secondary' && finalSCount === 0) {
            // Secondary empty; switch to group with tabs
            if (finalPCount > 0) {
              draft.activeGroupId = 'primary';
            } else if (finalTCount > 0) {
              draft.activeGroupId = 'tertiary';
            }
          } else if (draft.activeGroupId === 'tertiary' && finalTCount === 0) {
            // Tertiary empty; switch to group with tabs
            if (finalPCount > 0) {
              draft.activeGroupId = 'primary';
            } else if (finalSCount > 0) {
              draft.activeGroupId = 'secondary';
            }
          }
        });
      },

      closeTerminalTabBySessionId: (sessionId) => {
        const state = get();
        const result = state.findTabByMetadata({ sessionId });
        if (!result || result.tab.content.type !== 'terminal') return;
        state.closeTab(result.tab.id, result.groupId, { forceRemove: true });
      },

      renameTerminalTabBySessionId: (sessionId, newName) => {
        const result = get().findTabByMetadata({ sessionId });
        if (!result || result.tab.content.type !== 'terminal') return;
        
        set((draft) => {
          const group = getGroup(draft, result.groupId);
          const tab = group.tabs.find(t => t.id === result.tab.id);
          if (tab) {
            const displayTitle = newName.length > 20 ? `${newName.slice(0, 20)}...` : newName;
            tab.title = displayTitle;
            tab.content.title = displayTitle;
            tab.content.data = { ...tab.content.data, sessionName: newName };
          }
        });
      },
      
      closeAllTabs: (groupId) => {
        set((draft) => {
          // grid9: run the dedicated lifecycle and never enter the legacy
          // three-field degradation matrix.
          if (draft.layout.splitMode === 'grid9') {
            if (groupId) {
              const group = getGroup(draft, groupId);
              keepPinnedTabsOnly(group);
              collapseTrailingGrid9(draft.layout);
              fixGrid9Active(draft);
            } else {
              grid9CloseAllCells(draft);
            }
            return;
          }

          if (groupId) {
            const group = getGroup(draft, groupId);
            keepPinnedTabsOnly(group);

            const pCount = draft.primaryGroup.tabs.filter(t => !t.isHidden).length;
            const sCount = draft.secondaryGroup.tabs.filter(t => !t.isHidden).length;

            if (draft.layout.splitMode === 'grid') {
              if (groupId === 'tertiary') {
                if (pCount > 0 && sCount > 0) {
                  draft.layout.splitMode = 'horizontal';
                  draft.activeGroupId = 'primary';
                } else if (pCount > 0 || sCount > 0) {
                  draft.primaryGroup = pCount > 0 ? draft.primaryGroup : draft.secondaryGroup;
                  draft.secondaryGroup = createEditorGroupState();
                  draft.tertiaryGroup = createEditorGroupState();
                  draft.layout.splitMode = 'none';
                  draft.activeGroupId = 'primary';
                } else {
                  draft.layout.splitMode = 'none';
                  draft.activeGroupId = 'primary';
                }
              } else {
                // Closing primary or secondary
                const tCount = draft.tertiaryGroup.tabs.filter(t => !t.isHidden).length;
                
                if (groupId === 'primary') {
                  // Closing primary; remaining secondary and/or tertiary
                  if (sCount > 0 && tCount > 0) {
                    // Secondary + tertiary remain -> downgrade to vertical
                    draft.primaryGroup = { ...draft.secondaryGroup };
                    draft.secondaryGroup = { ...draft.tertiaryGroup };
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'vertical';
                    draft.activeGroupId = 'primary';
                  } else if (sCount > 0) {
                    // Only secondary remains
                    draft.primaryGroup = { ...draft.secondaryGroup };
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else if (tCount > 0) {
                    // Only tertiary remains
                    draft.primaryGroup = { ...draft.tertiaryGroup };
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else {
                    // All empty
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  }
                } else if (groupId === 'secondary') {
                  // Closing secondary; remaining primary and/or tertiary
                  if (pCount > 0 && tCount > 0) {
                    // Primary + tertiary remain -> downgrade to vertical
                    draft.secondaryGroup = { ...draft.tertiaryGroup };
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'vertical';
                    draft.activeGroupId = 'primary';
                  } else if (pCount > 0) {
                    // Only primary remains
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else if (tCount > 0) {
                    // Only tertiary remains
                    draft.primaryGroup = { ...draft.tertiaryGroup };
                    draft.secondaryGroup = createEditorGroupState();
                    draft.tertiaryGroup = createEditorGroupState();
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  } else {
                    // All empty
                    draft.layout.splitMode = 'none';
                    draft.activeGroupId = 'primary';
                  }
                }
              }
            } else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
              // Handle horizontal/vertical split mode
              if (groupId === 'secondary' && pCount > 0) {
                // Close secondary; primary has tabs -> merge to single column
                draft.secondaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
                // Ensure primary has a valid activeTabId
                const visibleTabs = draft.primaryGroup.tabs.filter(t => !t.isHidden);
                if (visibleTabs.length > 0 && (!draft.primaryGroup.activeTabId || !visibleTabs.find(t => t.id === draft.primaryGroup.activeTabId))) {
                  draft.primaryGroup.activeTabId = visibleTabs[0].id;
                }
              } else if (groupId === 'primary' && sCount > 0) {
                // Close primary; secondary has tabs -> move to primary
                draft.primaryGroup = { ...draft.secondaryGroup };
                draft.secondaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              } else {
                // Both groups empty or closing the only group with tabs
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
              }
            }
          } else {
            keepPinnedTabsOnly(draft.primaryGroup);
            keepPinnedTabsOnly(draft.secondaryGroup);
            keepPinnedTabsOnly(draft.tertiaryGroup);

            const pCount = getVisibleCount(draft.primaryGroup);
            const sCount = getVisibleCount(draft.secondaryGroup);
            const tCount = getVisibleCount(draft.tertiaryGroup);

            if (pCount === 0 && sCount === 0 && tCount === 0) {
              draft.primaryGroup = createEditorGroupState();
              draft.secondaryGroup = createEditorGroupState();
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (draft.layout.splitMode === 'grid') {
              if (pCount > 0 && sCount > 0 && tCount > 0) {
                ensureValidActiveTab(draft.primaryGroup);
                ensureValidActiveTab(draft.secondaryGroup);
                ensureValidActiveTab(draft.tertiaryGroup);
              } else {
                const remainingGroups: EditorGroupState[] = [];
                if (pCount > 0) remainingGroups.push(draft.primaryGroup);
                if (sCount > 0) remainingGroups.push(draft.secondaryGroup);
                if (tCount > 0) remainingGroups.push(draft.tertiaryGroup);

                draft.primaryGroup = remainingGroups[0] ? { ...remainingGroups[0] } : createEditorGroupState();
                draft.secondaryGroup = remainingGroups[1] ? { ...remainingGroups[1] } : createEditorGroupState();
                draft.tertiaryGroup = remainingGroups[2] ? { ...remainingGroups[2] } : createEditorGroupState();
                draft.layout.splitMode = remainingGroups.length >= 3 ? 'grid' : remainingGroups.length === 2 ? 'horizontal' : 'none';
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
                ensureValidActiveTab(draft.secondaryGroup);
                ensureValidActiveTab(draft.tertiaryGroup);
              }
            } else if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
              if (pCount > 0 && sCount > 0) {
                ensureValidActiveTab(draft.primaryGroup);
                ensureValidActiveTab(draft.secondaryGroup);
              } else if (pCount > 0) {
                draft.secondaryGroup = createEditorGroupState();
                draft.tertiaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
              } else if (sCount > 0) {
                draft.primaryGroup = { ...draft.secondaryGroup };
                draft.secondaryGroup = createEditorGroupState();
                draft.tertiaryGroup = createEditorGroupState();
                draft.layout.splitMode = 'none';
                draft.activeGroupId = 'primary';
                ensureValidActiveTab(draft.primaryGroup);
              }
            } else {
              ensureValidActiveTab(draft.primaryGroup);
              draft.activeGroupId = 'primary';
            }
          }
        });
      },
      
      switchToTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (!tab) return;
          
          // Unhide if the tab is hidden
          if (tab.isHidden) {
            tab.isHidden = false;
          }
          
          // Update last accessed time
          tab.lastAccessedAt = Date.now();
          
          group.activeTabId = tabId;
          draft.activeGroupId = groupId;
        });
      },
      
      updateTabContent: (tabId, groupId, content) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.content = content;
            tab.title = content.title || tab.title;
          }
        });
      },
      
      setTabDirty: (tabId, groupId, isDirty) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.isDirty = isDirty;
          }
        });
      },

      setTabFileDeletedFromDisk: (tabId, groupId, deleted) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          if (tab) {
            tab.fileDeletedFromDisk = deleted;
          }
        });
      },
      
      promoteTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab && tab.state === 'preview') {
            tab.state = 'active';
          }
        });
      },
      
      togglePinTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            if (tab.state === 'pinned') {
              tab.state = 'active';
            } else {
              tab.state = 'pinned';
            }

            const tabIndex = group.tabs.findIndex(t => t.id === tabId);
            if (tabIndex !== -1) {
              const [movedTab] = group.tabs.splice(tabIndex, 1);
              insertTabRespectingPinnedBoundary(group, movedTab);
            }
          }
        });
      },
      
      findTabByMetadata: (metadata) => {
        const state = get();
        const groups: { id: EditorGroupId; group: EditorGroupState }[] = [
          { id: 'primary', group: state.primaryGroup },
          { id: 'secondary', group: state.secondaryGroup },
          { id: 'tertiary', group: state.tertiaryGroup },
        ];
        
        for (const { id, group } of groups) {
          const tab = group.tabs.find(t => {
            if (!t.content.metadata) return false;
            return Object.keys(metadata).every(key => {
              const metadataValue = metadata[key];
              const tabValue = t.content.metadata?.[key];
              if (key === 'duplicateCheckKey' && typeof metadataValue === 'string' && typeof tabValue === 'string') {
                return normalizePath(metadataValue) === normalizePath(tabValue);
              }
              return tabValue === metadataValue;
            });
          });
          if (tab) {
            return { tab, groupId: id };
          }
        }
        return null;
      },
      
      reopenClosedTab: () => {
        set((draft) => {
          const record = draft.closedTabs.shift();
          if (record) {
            // grid9: if the recorded slot no longer exists (was removed/merged),
            // restore into the primary cell instead of a dead slot.
            if (draft.layout.splitMode === 'grid9') {
              const gid = record.groupId;
              const cellExists = !!draft.layout.grid9Cells[gid];
              const insideTemplate =
                EDITOR_GROUP_ROW[gid] < draft.layout.grid9RowsCount &&
                EDITOR_GROUP_COL[gid] < draft.layout.grid9ColsCount;
              const targetGid = cellExists && insideTemplate ? gid : 'primary';
              const cell = draft.layout.grid9Cells[targetGid] ?? createEditorGroupState();
              const insertIndex = Math.min(record.index, cell.tabs.length);
              cell.tabs.splice(insertIndex, 0, {
                ...record.tab,
                lastAccessedAt: Date.now(),
              });
              cell.activeTabId = record.tab.id;
              draft.layout.grid9Cells[targetGid] = cell;
              draft.activeGroupId = targetGid;
              return;
            }

            const group = getGroup(draft, record.groupId);
            
            // Restore tab to its original position
            const insertIndex = Math.min(record.index, group.tabs.length);
            group.tabs.splice(insertIndex, 0, {
              ...record.tab,
              lastAccessedAt: Date.now(),
            });
            group.activeTabId = record.tab.id;
            draft.activeGroupId = record.groupId;
          }
        });
      },
      
      hideTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.isHidden = true;
            
            if (group.activeTabId === tabId) {
              const visibleTabs = group.tabs.filter(t => !t.isHidden);
              group.activeTabId = visibleTabs[0]?.id || null;
            }
          }
        });
      },
      
      showTab: (tabId, groupId) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tab = group.tabs.find(t => t.id === tabId);
          
          if (tab) {
            tab.isHidden = false;
            group.activeTabId = tabId;
          }
        });
      },
      
      // ==================== Drag Operations ====================
      
      startDrag: (tabId, groupId) => {
        set((draft) => {
          draft.draggingTabId = tabId;
          draft.draggingFromGroupId = groupId;
        });
      },
      
      endDrag: () => {
        set((draft) => {
          draft.draggingTabId = null;
          draft.draggingFromGroupId = null;
        });
      },
      
      moveTabToGroup: (tabId, fromGroupId, toGroupId, index) => {
        if (fromGroupId === toGroupId) return;
        
        set((draft) => {
          const fromGroup = fromGroupId === 'primary' ? draft.primaryGroup : draft.secondaryGroup;
          const toGroup = toGroupId === 'primary' ? draft.primaryGroup : draft.secondaryGroup;
          
          const tabIndex = fromGroup.tabs.findIndex(t => t.id === tabId);
          if (tabIndex === -1) return;
          
          const [tab] = fromGroup.tabs.splice(tabIndex, 1);
          
          // Add to target group
          const insertIndex = index !== undefined ? Math.min(index, toGroup.tabs.length) : 0;
          toGroup.tabs.splice(insertIndex, 0, tab);
          toGroup.activeTabId = tab.id;
          
          // Update active tab in source group
          if (fromGroup.activeTabId === tabId) {
            const visibleTabs = fromGroup.tabs.filter(t => !t.isHidden);
            fromGroup.activeTabId = visibleTabs[Math.min(tabIndex, visibleTabs.length - 1)]?.id || null;
          }
          
          // If single-column, enable split
          if (draft.layout.splitMode === 'none') {
            draft.layout.splitMode = 'horizontal';
          }
          
          draft.activeGroupId = toGroupId;
        });
      },
      
      reorderTab: (tabId, groupId, newIndex) => {
        set((draft) => {
          const group = getGroup(draft, groupId);
          const tabIndex = group.tabs.findIndex(t => t.id === tabId);
          
          if (tabIndex === -1 || tabIndex === newIndex) return;
          
          const [tab] = group.tabs.splice(tabIndex, 1);
          const pinnedBoundary = getPinnedBoundary(group);
          const targetIndex = tab.state === 'pinned'
            ? Math.max(0, Math.min(newIndex, pinnedBoundary))
            : Math.max(pinnedBoundary, Math.min(newIndex, group.tabs.length));
          group.tabs.splice(targetIndex, 0, tab);
        });
      },
      
      handleDrop: (tabId, fromGroupId, toGroupId, position) => {
        set((draft) => {
          const fromGroup = getGroup(draft, fromGroupId);
          const tabIndex = fromGroup.tabs.findIndex(t => t.id === tabId);
          if (tabIndex === -1) return;

          const [tab] = fromGroup.tabs.splice(tabIndex, 1);

          if (fromGroup.activeTabId === tabId) {
            const visible = fromGroup.tabs.filter(t => !t.isHidden);
            fromGroup.activeTabId = visible[Math.min(tabIndex, visible.length - 1)]?.id || null;
          }

          const { splitMode } = draft.layout;

          if (splitMode === 'none') {
            if (position === 'left' || position === 'right') {
              draft.layout.splitMode = 'horizontal';
              if (position === 'left') {
                draft.secondaryGroup.tabs = [...draft.primaryGroup.tabs];
                draft.secondaryGroup.activeTabId = draft.primaryGroup.activeTabId;
                draft.primaryGroup.tabs = [tab];
                draft.primaryGroup.activeTabId = tab.id;
              } else {
                draft.secondaryGroup.tabs = [tab];
                draft.secondaryGroup.activeTabId = tab.id;
              }
              draft.activeGroupId = position === 'left' ? 'primary' : 'secondary';
            } else if (position === 'top' || position === 'bottom') {
              draft.layout.splitMode = 'vertical';
              if (position === 'top') {
                draft.secondaryGroup.tabs = [...draft.primaryGroup.tabs];
                draft.secondaryGroup.activeTabId = draft.primaryGroup.activeTabId;
                draft.primaryGroup.tabs = [tab];
                draft.primaryGroup.activeTabId = tab.id;
              } else {
                draft.secondaryGroup.tabs = [tab];
                draft.secondaryGroup.activeTabId = tab.id;
              }
              draft.activeGroupId = position === 'top' ? 'primary' : 'secondary';
            }
          } else if (splitMode === 'horizontal') {
            if (position === 'bottom') {
              draft.layout.splitMode = 'grid';
              draft.tertiaryGroup.tabs = [tab];
              draft.tertiaryGroup.activeTabId = tab.id;
              draft.activeGroupId = 'tertiary';
            } else if (position === 'top') {
              draft.layout.splitMode = 'grid';
              draft.tertiaryGroup.tabs = [...draft.primaryGroup.tabs, ...draft.secondaryGroup.tabs];
              draft.tertiaryGroup.activeTabId = draft.primaryGroup.activeTabId || draft.secondaryGroup.activeTabId;
              draft.primaryGroup.tabs = [tab];
              draft.primaryGroup.activeTabId = tab.id;
              draft.secondaryGroup = createEditorGroupState();
              draft.activeGroupId = 'primary';
            } else if (position === 'center') {
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            } else {
              const targetGroupId = position === 'left' ? 'primary' : 'secondary';
              const targetGroup = getGroup(draft, targetGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = targetGroupId;
            }
          } else if (splitMode === 'vertical') {
            if (position === 'center') {
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            } else {
              const targetGroupId = position === 'top' ? 'primary' : 'secondary';
              const targetGroup = getGroup(draft, targetGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = targetGroupId;
            }
          } else if (splitMode === 'grid') {
            if (position === 'bottom' && toGroupId === 'tertiary') {
              // Expand the 3-pane (left/right/bottom) into the grid: the dragged
              // tab opens row 1 (rows grows to 2), keeping the existing 2
              // columns. Rows/columns stay independent. The new cell below
              // tertiary is row1 col1 (slot6 in row-major), computed from the
              // grid geometry so it stays correct if the geometry changes.
              seedGrid9CellsFromLegacy(draft);
              draft.layout.splitMode = 'grid9';
              draft.layout.grid9ColsCount = 2;
              draft.layout.grid9RowsCount = 2;
              resetGrid9Ratios(draft.layout, 2, 2);
              const slotId = EDITOR_GROUP_IDS[1 * GRID_MAX_DIM + 1]; // slot6
              const slotGroup = getGroup(draft, slotId);
              slotGroup.tabs = [tab];
              slotGroup.activeTabId = tab.id;
              draft.activeGroupId = slotId;
            } else if (position === 'center') {
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            }
          } else if (splitMode === 'grid9') {
            // grid9 with independent rows/columns (grid9ColsCount ×
            // grid9RowsCount, each 1..GRID_MAX_DIM). Edge drops grow the
            // corresponding axis; the center drop places the tab into the
            // target slot and grows the grid if that slot is outside it.
            const targetRow = EDITOR_GROUP_ROW[toGroupId];
            const targetCol = EDITOR_GROUP_COL[toGroupId];
            if (position === 'left' || position === 'right') {
              if (draft.layout.grid9ColsCount < GRID_MAX_DIM) {
                draft.layout.grid9ColsCount += 1;
              }
              draft.layout.grid9ColRatios = growGrid9RatiosTo(
                draft.layout.grid9ColRatios,
                draft.layout.grid9ColsCount,
              );
              const newCol = Math.min(draft.layout.grid9ColsCount - 1, GRID_MAX_DIM - 1);
              const slotId = EDITOR_GROUP_IDS[targetRow * GRID_MAX_DIM + newCol];
              const slotGroup = getGroup(draft, slotId);
              slotGroup.tabs.unshift(tab);
              slotGroup.activeTabId = tab.id;
              draft.activeGroupId = slotId;
            } else if (position === 'top' || position === 'bottom') {
              if (draft.layout.grid9RowsCount < GRID_MAX_DIM) {
                draft.layout.grid9RowsCount += 1;
              }
              draft.layout.grid9RowRatios = growGrid9RatiosTo(
                draft.layout.grid9RowRatios,
                draft.layout.grid9RowsCount,
              );
              const newRow = Math.min(draft.layout.grid9RowsCount - 1, GRID_MAX_DIM - 1);
              const slotId = EDITOR_GROUP_IDS[newRow * GRID_MAX_DIM + targetCol];
              const slotGroup = getGroup(draft, slotId);
              slotGroup.tabs.unshift(tab);
              slotGroup.activeTabId = tab.id;
              draft.activeGroupId = slotId;
            } else {
              // center: place into the target slot; grow the grid if the slot is
              // outside the current rows/cols.
              if (targetRow >= draft.layout.grid9RowsCount) {
                draft.layout.grid9RowsCount = targetRow + 1;
              }
              if (targetCol >= draft.layout.grid9ColsCount) {
                draft.layout.grid9ColsCount = targetCol + 1;
              }
              draft.layout.grid9ColRatios = growGrid9RatiosTo(
                draft.layout.grid9ColRatios,
                draft.layout.grid9ColsCount,
              );
              draft.layout.grid9RowRatios = growGrid9RatiosTo(
                draft.layout.grid9RowRatios,
                draft.layout.grid9RowsCount,
              );
              const targetGroup = getGroup(draft, toGroupId);
              targetGroup.tabs.unshift(tab);
              targetGroup.activeTabId = tab.id;
              draft.activeGroupId = toGroupId;
            }
          }

          // grid9: no auto-merge/downgrade — just collapse trailing empty rows
          // and columns and keep activeGroupId on a live cell.
          if (draft.layout.splitMode === 'grid9') {
            collapseTrailingGrid9(draft.layout);
            fixGrid9Active(draft);
            return;
          }

          // Auto-merge empty editor groups
          const getVisibleCount = (g: EditorGroupState) => g.tabs.filter(t => !t.isHidden).length;
          const primaryCount = getVisibleCount(draft.primaryGroup);
          const secondaryCount = getVisibleCount(draft.secondaryGroup);
          const tertiaryCount = getVisibleCount(draft.tertiaryGroup);

          if (draft.layout.splitMode === 'grid') {
            let gridHandled = false;
            
            if (tertiaryCount === 0) {
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'horizontal';
              gridHandled = true;
            }
            if (primaryCount === 0 && secondaryCount === 0) {
              draft.primaryGroup = { ...draft.tertiaryGroup };
              draft.secondaryGroup = createEditorGroupState();
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
              gridHandled = true;
            }
            // FIX: handle primary empty while secondary and tertiary have tabs
            if (primaryCount === 0 && secondaryCount > 0 && tertiaryCount > 0) {
              // Move secondary -> primary (top), tertiary -> secondary (bottom), downgrade to vertical
              // Tabs are dropped to "bottom", so final layout should be vertical
              draft.primaryGroup = { ...draft.secondaryGroup };
              draft.secondaryGroup = { ...draft.tertiaryGroup };
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';
              // If active group is tertiary, update to secondary
              if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
              gridHandled = true;
            }
            // FIX: handle secondary empty while primary and tertiary have tabs
            if (secondaryCount === 0 && primaryCount > 0 && tertiaryCount > 0) {
              // Move tertiary -> secondary, downgrade to vertical
              // Primary (top-left) and tertiary (bottom) are vertical
              draft.secondaryGroup = { ...draft.tertiaryGroup };
              draft.tertiaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'vertical';
              // If active group is tertiary, update to secondary
              if (draft.activeGroupId === 'tertiary') {
                draft.activeGroupId = 'secondary';
              }
              gridHandled = true;
            }
            
            // If grid handling finished, skip horizontal/vertical checks
            if (gridHandled) {
              return;
            }
          }

          if (draft.layout.splitMode === 'horizontal' || draft.layout.splitMode === 'vertical') {
            if (secondaryCount === 0) {
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            } else if (primaryCount === 0) {
              draft.primaryGroup = { ...draft.secondaryGroup };
              draft.secondaryGroup = createEditorGroupState();
              draft.layout.splitMode = 'none';
              draft.activeGroupId = 'primary';
            }
          }
        });
      },
      
      // ==================== Layout Operations ====================
      
      setSplitMode: (mode) => {
        set((draft) => {
          if (mode === 'grid9' && draft.layout.splitMode !== 'grid9') {
            // Enter grid9: seed grid9Cells from the legacy content and clamp the
            // active counts to 1x1 (the drop/template ops grow them from here).
            seedGrid9CellsFromLegacy(draft);
            draft.layout.grid9ColsCount = 1;
            draft.layout.grid9RowsCount = 1;
            draft.layout.grid9ColRatios = [1];
            draft.layout.grid9RowRatios = [1];
          } else if (mode === 'none' && draft.layout.splitMode === 'grid9') {
            // Leave grid9: collect every grid9 cell's tabs into primary so no
            // tab is lost, then drop back to a single column.
            const allTabs: CanvasTab[] = [];
            for (const gid of EDITOR_GROUP_IDS) {
              const cell = draft.layout.grid9Cells[gid];
              if (cell) allTabs.push(...cell.tabs);
            }
            draft.primaryGroup.tabs = allTabs;
            draft.primaryGroup.activeTabId = allTabs[0]?.id ?? null;
            draft.layout.grid9Cells = {};
          } else if (mode === 'none' && draft.layout.splitMode !== 'none') {
            const allTabs = [
              ...draft.primaryGroup.tabs,
              ...draft.secondaryGroup.tabs,
              ...draft.tertiaryGroup.tabs,
            ];
            draft.primaryGroup.tabs = allTabs;
            draft.primaryGroup.activeTabId = 
              draft.primaryGroup.activeTabId || 
              draft.secondaryGroup.activeTabId || 
              draft.tertiaryGroup.activeTabId;
            draft.secondaryGroup = createEditorGroupState();
            draft.tertiaryGroup = createEditorGroupState();
            draft.activeGroupId = 'primary';
          }
          draft.layout.splitMode = mode;
        });
      },
      
      setSplitRatio: (ratio) => {
        set((draft) => {
          draft.layout.splitRatio = clampSplitRatio(ratio);
        });
      },

      setSplitRatio2: (ratio) => {
        set((draft) => {
          draft.layout.splitRatio2 = clampSplitRatio(ratio);
        });
      },

      // ==================== Grid9 templates & operations ====================

      enterGrid9: (cols, rows) => {
        set((draft) => {
          const wasGrid9 = draft.layout.splitMode === 'grid9';
          if (!wasGrid9) {
            seedGrid9CellsFromLegacy(draft);
          }
          const c = clampGrid9Dim(cols);
          const r = clampGrid9Dim(rows);
          draft.layout.splitMode = 'grid9';
          draft.layout.grid9ColsCount = c;
          draft.layout.grid9RowsCount = r;
          if (draft.layout.grid9ColRatios.length !== c) {
            draft.layout.grid9ColRatios = growGrid9RatiosTo(draft.layout.grid9ColRatios, c);
          }
          if (draft.layout.grid9RowRatios.length !== r) {
            draft.layout.grid9RowRatios = growGrid9RatiosTo(draft.layout.grid9RowRatios, r);
          }
        });
      },

      applyGrid9Template: (cols, rows) => {
        set((draft) => {
          const wasGrid9 = draft.layout.splitMode === 'grid9';
          if (!wasGrid9) {
            seedGrid9CellsFromLegacy(draft);
          }
          const c = clampGrid9Dim(cols);
          const r = clampGrid9Dim(rows);
          draft.layout.splitMode = 'grid9';
          draft.layout.grid9ColsCount = c;
          draft.layout.grid9RowsCount = r;
          // A template always tiles evenly: reset ratios to equal shares.
          resetGrid9Ratios(draft.layout, c, r);

          // Move any tabs from slots outside the new template into the primary
          // cell (never silently dropped).
          const orphanedTabs: CanvasTab[] = [];
          for (const gid of EDITOR_GROUP_IDS) {
            const row = EDITOR_GROUP_ROW[gid];
            const col = EDITOR_GROUP_COL[gid];
            if (row >= r || col >= c) {
              const slot = draft.layout.grid9Cells[gid];
              if (slot && slot.tabs.length > 0) {
                if (slot.activeTabId && slot.tabs.some(t => t.id === slot.activeTabId)) {
                  draft.layout.grid9Cells['primary'] = {
                    ...(draft.layout.grid9Cells['primary'] ?? createEditorGroupState()),
                    activeTabId: slot.activeTabId,
                  };
                }
                orphanedTabs.push(...slot.tabs);
              }
              delete draft.layout.grid9Cells[gid];
            }
          }
          if (orphanedTabs.length > 0) {
            const primaryCell = draft.layout.grid9Cells['primary'] ?? createEditorGroupState();
            primaryCell.tabs = [...primaryCell.tabs, ...orphanedTabs];
            if (!primaryCell.activeTabId) {
              primaryCell.activeTabId = primaryCell.tabs[0]?.id ?? null;
            }
            draft.layout.grid9Cells['primary'] = primaryCell;
          }

          // Keep activeGroupId inside the new template.
          const activeRow = EDITOR_GROUP_ROW[draft.activeGroupId];
          const activeCol = EDITOR_GROUP_COL[draft.activeGroupId];
          if (
            activeRow >= r ||
            activeCol >= c ||
            !draft.layout.grid9Cells[draft.activeGroupId]
          ) {
            draft.activeGroupId = 'primary';
          }
          const primaryCell = draft.layout.grid9Cells['primary'];
          if (primaryCell && primaryCell.tabs.length > 0 && !primaryCell.activeTabId) {
            primaryCell.activeTabId = primaryCell.tabs[0].id;
          }
        });
      },

      mergeGrid9Cells: (fromGroupId, toGroupId) => {
        set((draft) => {
          if (fromGroupId === toGroupId) return;
          const source = draft.layout.grid9Cells[fromGroupId];
          if (!source || source.tabs.length === 0) return;
          const target = draft.layout.grid9Cells[toGroupId] ?? createEditorGroupState();
          if (source.activeTabId && source.tabs.some(t => t.id === source.activeTabId)) {
            target.activeTabId = source.activeTabId;
          }
          target.tabs = [...target.tabs, ...source.tabs];
          draft.layout.grid9Cells[toGroupId] = target;
          delete draft.layout.grid9Cells[fromGroupId];
          draft.activeGroupId = toGroupId;
        });
      },

      removeGrid9Cell: (groupId) => {
        set((draft) => {
          const layout = draft.layout;
          if (layout.splitMode !== 'grid9') return;
          if (EDITOR_GROUP_IDS.indexOf(groupId) < 0) return;
          const row = EDITOR_GROUP_ROW[groupId];
          const col = EDITOR_GROUP_COL[groupId];
          const cols = layout.grid9ColsCount;
          const rows = layout.grid9RowsCount;
          // A 1x1 grid cannot shrink any further.
          if (cols <= 1 && rows <= 1) return;

          const moveTabs = (fromGid: EditorGroupId, toGid: EditorGroupId) => {
            const from = layout.grid9Cells[fromGid];
            if (!from || from.tabs.length === 0) return;
            const to = layout.grid9Cells[toGid] ?? createEditorGroupState();
            if (from.activeTabId && from.tabs.some(t => t.id === from.activeTabId)) {
              to.activeTabId = from.activeTabId;
            }
            to.tabs = [...to.tabs, ...from.tabs];
            layout.grid9Cells[toGid] = to;
            delete layout.grid9Cells[fromGid];
          };
          const resetCell = (gid: EditorGroupId) => {
            delete layout.grid9Cells[gid];
          };

          if (cols > 1) {
            const mergeTargetCol = col > 0 ? col - 1 : 1;
            for (let r = 0; r < rows; r++) {
              moveTabs(
                EDITOR_GROUP_IDS[r * GRID_MAX_DIM + col],
                EDITOR_GROUP_IDS[r * GRID_MAX_DIM + mergeTargetCol],
              );
            }
            for (let r = 0; r < rows; r++) {
              for (let c = col === 0 ? 0 : col; c < cols - 1; c++) {
                moveTabs(
                  EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c + 1],
                  EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c],
                );
              }
              resetCell(EDITOR_GROUP_IDS[r * GRID_MAX_DIM + cols - 1]);
            }
            layout.grid9ColsCount = cols - 1;
            layout.grid9ColRatios = removeGrid9RatioAt(layout.grid9ColRatios, col);
          } else {
            const mergeTargetRow = row > 0 ? row - 1 : 1;
            for (let c = 0; c < cols; c++) {
              moveTabs(
                EDITOR_GROUP_IDS[row * GRID_MAX_DIM + c],
                EDITOR_GROUP_IDS[mergeTargetRow * GRID_MAX_DIM + c],
              );
            }
            for (let c = 0; c < cols; c++) {
              for (let r = row === 0 ? 0 : row; r < rows - 1; r++) {
                moveTabs(
                  EDITOR_GROUP_IDS[(r + 1) * GRID_MAX_DIM + c],
                  EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c],
                );
              }
              resetCell(EDITOR_GROUP_IDS[(rows - 1) * GRID_MAX_DIM + c]);
            }
            layout.grid9RowsCount = rows - 1;
            layout.grid9RowRatios = removeGrid9RatioAt(layout.grid9RowRatios, row);
          }

          // Clear any leftover cells outside the new template (their content was
          // already relocated), then keep activeGroupId inside the template.
          const newCols = layout.grid9ColsCount;
          const newRows = layout.grid9RowsCount;
          for (const gid of EDITOR_GROUP_IDS) {
            if (EDITOR_GROUP_ROW[gid] >= newRows || EDITOR_GROUP_COL[gid] >= newCols) {
              const cell = layout.grid9Cells[gid];
              if (cell && cell.tabs.length > 0) {
                // Defensive: never drop tabs that ended up outside the template.
                const primaryCell = layout.grid9Cells['primary'] ?? createEditorGroupState();
                primaryCell.tabs = [...primaryCell.tabs, ...cell.tabs];
                layout.grid9Cells['primary'] = primaryCell;
              }
              delete layout.grid9Cells[gid];
            }
          }

          const activeRow = EDITOR_GROUP_ROW[draft.activeGroupId];
          const activeCol = EDITOR_GROUP_COL[draft.activeGroupId];
          const activeCell = layout.grid9Cells[draft.activeGroupId];
          if (
            activeRow >= newRows ||
            activeCol >= newCols ||
            !activeCell ||
            getVisibleCount(activeCell) === 0
          ) {
            const firstNonEmpty = EDITOR_GROUP_IDS.find(gid => {
              const cell = layout.grid9Cells[gid];
              return (
                EDITOR_GROUP_ROW[gid] < newRows &&
                EDITOR_GROUP_COL[gid] < newCols &&
                !!cell &&
                getVisibleCount(cell) > 0
              );
            });
            draft.activeGroupId = firstNonEmpty ?? 'primary';
          }
          const primaryCell = layout.grid9Cells['primary'];
          if (primaryCell && primaryCell.tabs.length > 0 && !primaryCell.activeTabId) {
            primaryCell.activeTabId = primaryCell.tabs[0].id;
          }
        });
      },

      setGrid9ColRatio: (col, ratio) => {
        set((draft) => {
          const ratios = draft.layout.grid9ColRatios;
          if (col < 0 || col >= ratios.length) return;
          const clamped = clampGrid9Ratio(ratio);
          if (ratios.length === 1) {
            ratios[0] = 1;
            return;
          }
          const others = ratios.length - 1;
          const sumOthers = ratios.reduce((acc, r, i) => (i === col ? acc : acc + r), 0);
          const scale = sumOthers === 0 ? 1 / others : (1 - clamped) / sumOthers;
          for (let i = 0; i < ratios.length; i++) {
            if (i === col) {
              ratios[i] = clamped;
            } else {
              ratios[i] *= scale;
            }
          }
        });
      },

      setGrid9RowRatio: (row, ratio) => {
        set((draft) => {
          const ratios = draft.layout.grid9RowRatios;
          if (row < 0 || row >= ratios.length) return;
          const clamped = clampGrid9Ratio(ratio);
          if (ratios.length === 1) {
            ratios[0] = 1;
            return;
          }
          const others = ratios.length - 1;
          const sumOthers = ratios.reduce((acc, r, i) => (i === row ? acc : acc + r), 0);
          const scale = sumOthers === 0 ? 1 / others : (1 - clamped) / sumOthers;
          for (let i = 0; i < ratios.length; i++) {
            if (i === row) {
              ratios[i] = clamped;
            } else {
              ratios[i] *= scale;
            }
          }
        });
      },

      getAllRenderableGroups: () => {
        const state = get();
        const groups: { id: EditorGroupId; group: EditorGroupState }[] = [];
        const pushIfRenderable = (id: EditorGroupId, group: EditorGroupState) => {
          if (group && getVisibleTabs(group).length > 0) groups.push({ id, group });
        };
        if (state.layout.splitMode === 'grid9') {
          for (const gid of EDITOR_GROUP_IDS) {
            const cell = state.layout.grid9Cells[gid];
            if (cell) pushIfRenderable(gid, cell);
          }
        } else {
          pushIfRenderable('primary', state.primaryGroup);
          pushIfRenderable('secondary', state.secondaryGroup);
          pushIfRenderable('tertiary', state.tertiaryGroup);
        }
        return groups;
      },
      
      setAnchorPosition: (position) => {
        set((draft) => {
          draft.layout.anchorPosition = position;
        });
      },
      
      setAnchorSize: (size) => {
        set((draft) => {
          draft.layout.anchorSize = clampAnchorSize(size);
        });
      },
      
      toggleMaximize: () => {
        set((draft) => {
          draft.layout.isMaximized = !draft.layout.isMaximized;
        });
      },
      
      setActiveGroup: (groupId) => {
        set((draft) => {
          draft.activeGroupId = groupId;
        });
      },
      
      // ==================== Mission Control ====================
      
      openMissionControl: () => {
        set((draft) => {
          draft.isMissionControlOpen = true;
        });
      },
      
      closeMissionControl: () => {
        set((draft) => {
          draft.isMissionControlOpen = false;
        });
      },
      
      toggleMissionControl: () => {
        set((draft) => {
          draft.isMissionControlOpen = !draft.isMissionControlOpen;
        });
      },
      
      // ==================== State Management ====================
      
      reset: () => {
        set(initialState);
      },
      
      getAllTabs: () => {
        const state = get();
        return [
          ...state.primaryGroup.tabs,
          ...state.secondaryGroup.tabs,
          ...state.tertiaryGroup.tabs,
        ];
      },
    }))
);

export type CanvasStoreMode = 'agent' | 'project' | 'git' | 'panel-view' | 'bottom-terminal';

/**
 * Selects which canvas store instance is used by the current subtree.
 * Defaults to 'agent' to preserve existing behavior in AI Agent scene.
 */
export const CanvasStoreModeContext = createContext<CanvasStoreMode>('agent');

export const useAgentCanvasStore = createCanvasStoreHook();
export const useProjectCanvasStore = createCanvasStoreHook();
export const useGitCanvasStore = createCanvasStoreHook();
export const usePanelViewCanvasStore = createCanvasStoreHook();
export const useBottomTerminalCanvasStore = createCanvasStoreHook();

// ==================== Agent canvas: per-workspace snapshots (AuxPane / Session scene) ====================
// Switching active workspace saves the current agent canvas under the previous workspace id and restores
// the snapshot for the next id, so remote/local tabs coexist across workspace switches.

const AGENT_CANVAS_SNAPSHOT_MAX = 12;
const agentWorkspaceSnapshots = new Map<string, CanvasStoreState>();
const agentSnapshotLruOrder: string[] = [];
/** Dedupes React Strict Mode double-invoke when `prev` is null (ref reset on remount). */
let lastAgentCanvasSwitchTargetKey: string | null = null;

function normalizeAgentWorkspaceKey(id: string | null | undefined): string {
  return id ?? '__none__';
}

function extractAgentPersistableState(state: CanvasStore): CanvasStoreState {
  return {
    primaryGroup: state.primaryGroup,
    secondaryGroup: state.secondaryGroup,
    tertiaryGroup: state.tertiaryGroup,
    activeGroupId: state.activeGroupId,
    layout: state.layout,
    isMissionControlOpen: state.isMissionControlOpen,
    draggingTabId: state.draggingTabId,
    draggingFromGroupId: state.draggingFromGroupId,
    closedTabs: state.closedTabs,
    maxClosedTabsHistory: state.maxClosedTabsHistory,
  };
}

function rememberAgentSnapshot(key: string, snapshot: CanvasStoreState): void {
  const clone = structuredClone(snapshot);
  clone.draggingTabId = null;
  clone.draggingFromGroupId = null;
  agentWorkspaceSnapshots.set(key, clone);
  const idx = agentSnapshotLruOrder.indexOf(key);
  if (idx >= 0) agentSnapshotLruOrder.splice(idx, 1);
  agentSnapshotLruOrder.push(key);
  while (agentWorkspaceSnapshots.size > AGENT_CANVAS_SNAPSHOT_MAX) {
    const evict = agentSnapshotLruOrder.shift();
    if (!evict) break;
    agentWorkspaceSnapshots.delete(evict);
  }
}

function applyEmptyAgentCanvas(): void {
  useAgentCanvasStore.setState({
    primaryGroup: createEditorGroupState(),
    secondaryGroup: createEditorGroupState(),
    tertiaryGroup: createEditorGroupState(),
    activeGroupId: 'primary',
    layout: createLayoutState(),
    isMissionControlOpen: false,
    draggingTabId: null,
    draggingFromGroupId: null,
    closedTabs: [],
    maxClosedTabsHistory: initialState.maxClosedTabsHistory,
  });
}

/** Clear agent canvas workspace snapshots when entering/exiting Peer Device Mode. */
export function clearAgentCanvasForPeerSwitch(): void {
  agentWorkspaceSnapshots.clear();
  agentSnapshotLruOrder.length = 0;
  lastAgentCanvasSwitchTargetKey = null;
  applyEmptyAgentCanvas();
  useProjectCanvasStore.getState().reset();
  useGitCanvasStore.getState().reset();
  usePanelViewCanvasStore.getState().reset();
  useBottomTerminalCanvasStore.getState().reset();
}

/**
 * Save the current agent canvas under `prevWorkspaceId` (unless first mount) and restore the snapshot
 * for `nextWorkspaceId` (or empty canvas if none). Capture target snapshot before LRU eviction.
 */
export function switchAgentCanvasWorkspace(
  prevWorkspaceId: string | null | undefined,
  nextWorkspaceId: string | null | undefined
): void {
  const from =
    prevWorkspaceId === null || prevWorkspaceId === undefined
      ? null
      : normalizeAgentWorkspaceKey(prevWorkspaceId);
  const to = normalizeAgentWorkspaceKey(nextWorkspaceId);

  if (from === null && lastAgentCanvasSwitchTargetKey === to) {
    return;
  }

  const rawNext = agentWorkspaceSnapshots.get(to);
  const nextSnapshotClone = rawNext ? structuredClone(rawNext) : null;

  if (from !== null) {
    const current = extractAgentPersistableState(useAgentCanvasStore.getState() as CanvasStore);
    rememberAgentSnapshot(from, current);
  }

  if (nextSnapshotClone) {
    useAgentCanvasStore.setState({
      primaryGroup: nextSnapshotClone.primaryGroup,
      secondaryGroup: nextSnapshotClone.secondaryGroup,
      tertiaryGroup: nextSnapshotClone.tertiaryGroup,
      activeGroupId: nextSnapshotClone.activeGroupId,
      layout: nextSnapshotClone.layout,
      isMissionControlOpen: false,
      draggingTabId: null,
      draggingFromGroupId: null,
      closedTabs: nextSnapshotClone.closedTabs,
      maxClosedTabsHistory: nextSnapshotClone.maxClosedTabsHistory,
    });
  } else {
    applyEmptyAgentCanvas();
  }

  lastAgentCanvasSwitchTargetKey = to;
}

/** Drop cached canvas for a closed workspace (does not touch the live canvas unless user switches back). */
export function removeAgentCanvasSnapshot(workspaceId: string): void {
  const key = normalizeAgentWorkspaceKey(workspaceId);
  agentWorkspaceSnapshots.delete(key);
  const idx = agentSnapshotLruOrder.indexOf(key);
  if (idx >= 0) agentSnapshotLruOrder.splice(idx, 1);
}

const selectWholeCanvasStore = (state: CanvasStore) => state;

export function useCanvasStore(): CanvasStore;
export function useCanvasStore<T>(selector: (state: CanvasStore) => T): T;
export function useCanvasStore<T>(selector?: (state: CanvasStore) => T): T | CanvasStore {
  const mode = useContext(CanvasStoreModeContext);
  const resolvedSelector = (selector ?? selectWholeCanvasStore) as (state: CanvasStore) => T | CanvasStore;

  // Keep hook order stable across mode switches by subscribing to each scoped store.
  const agentValue = useAgentCanvasStore(resolvedSelector);
  const projectValue = useProjectCanvasStore(resolvedSelector);
  const gitValue = useGitCanvasStore(resolvedSelector);
  const panelViewValue = usePanelViewCanvasStore(resolvedSelector);
  const bottomTerminalValue = useBottomTerminalCanvasStore(resolvedSelector);

  if (mode === 'project') return projectValue;
  if (mode === 'git') return gitValue;
  if (mode === 'panel-view') return panelViewValue;
  if (mode === 'bottom-terminal') return bottomTerminalValue;
  return agentValue;
}

// ==================== Selector Hooks ====================

/**
 * Get tabs for a specific editor group. In grid9 mode the slot lives in the
 * single grid9Cells map; otherwise the legacy three-field layout is used.
 */
export const useGroupTabs = (groupId: EditorGroupId) => {
  return useCanvasStore((state) => {
    if (state.layout.splitMode === 'grid9') {
      return state.layout.grid9Cells[groupId]?.tabs ?? [];
    }
    if (groupId === 'primary') return state.primaryGroup.tabs;
    if (groupId === 'secondary') return state.secondaryGroup.tabs;
    return state.tertiaryGroup.tabs;
  });
};

/**
 * Get active tab ID for a specific editor group (grid9 slot-aware).
 */
export const useActiveTabId = (groupId: EditorGroupId) => {
  return useCanvasStore((state) => {
    if (state.layout.splitMode === 'grid9') {
      return state.layout.grid9Cells[groupId]?.activeTabId ?? null;
    }
    if (groupId === 'primary') return state.primaryGroup.activeTabId;
    if (groupId === 'secondary') return state.secondaryGroup.activeTabId;
    return state.tertiaryGroup.activeTabId;
  });
};

/**
 * Get layout state.
 */
export const useLayout = () => {
  return useCanvasStore((state) => state.layout);
};

/**
 * Get drag state.
 */
export const useDragging = () => {
  return useCanvasStore((state) => ({
    draggingTabId: state.draggingTabId,
    draggingFromGroupId: state.draggingFromGroupId,
  }));
};

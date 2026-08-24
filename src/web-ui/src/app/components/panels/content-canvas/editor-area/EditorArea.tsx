import React, { useRef, useCallback } from 'react';
import { EditorGroup, type EditorGroupProps } from './EditorGroup';
import { SplitHandle } from './SplitHandle';
import { useCanvasStore } from '../stores';
import type { 
  EditorGroupId, 
  Grid9Slot,
  TabDragPayload, 
  DropPosition,
  PanelContent,
} from '../types';
import {
  EDITOR_GROUP_IDS,
  GRID_MAX_DIM,
  LAYOUT_CONFIG,
  GRID9_RATIO_CONFIG,
  createEditorGroupState,
} from '../types';
import './EditorArea.scss';

/** Grid9 cell-level grid props forwarded from EditorArea into each EditorGroup. */
type Grid9CellProps = Pick<EditorGroupProps, 'grid9Slot' | 'gridMerge' | 'gridRemove'>;

export interface EditorAreaProps {
  workspacePath?: string;
  isSceneActive?: boolean;
  onOpenMissionControl?: () => void;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  onTabCloseWithDirtyCheck?: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;
  onTabCloseAllWithDirtyCheck?: (groupId: EditorGroupId) => Promise<boolean>;
  disablePopOut?: boolean;
  terminalResizeSuspended?: boolean;
  /** Optional grid9 slot info threaded from ContentCanvas → EditorArea →
   *  EditorGroup → TabBar (primary only). If absent EditorArea builds one. */
  grid9Slot?: Grid9Slot;
}

export const EditorArea: React.FC<EditorAreaProps> = ({
  workspacePath,
  isSceneActive = true,
  onOpenMissionControl,
  onInteraction,
  onTabCloseWithDirtyCheck,
  onTabCloseAllWithDirtyCheck,
  disablePopOut = false,
  terminalResizeSuspended = false,
  grid9Slot,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const topRowRef = useRef<HTMLDivElement>(null);
  const grid9Ref = useRef<HTMLDivElement>(null);

  // Fine-grained selectors: subscribe to each slice/action individually so
  // unrelated store changes do not re-render the editor area.
  const primaryGroup = useCanvasStore(state => state.primaryGroup);
  const secondaryGroup = useCanvasStore(state => state.secondaryGroup);
  const tertiaryGroup = useCanvasStore(state => state.tertiaryGroup);
  const activeGroupId = useCanvasStore(state => state.activeGroupId);
  const layout = useCanvasStore(state => state.layout);
  const draggingTabId = useCanvasStore(state => state.draggingTabId);
  const draggingFromGroupId = useCanvasStore(state => state.draggingFromGroupId);
  const switchToTab = useCanvasStore(state => state.switchToTab);
  const closeTab = useCanvasStore(state => state.closeTab);
  const closeAllTabs = useCanvasStore(state => state.closeAllTabs);
  const promoteTab = useCanvasStore(state => state.promoteTab);
  const togglePinTab = useCanvasStore(state => state.togglePinTab);
  const startDrag = useCanvasStore(state => state.startDrag);
  const endDrag = useCanvasStore(state => state.endDrag);
  const reorderTab = useCanvasStore(state => state.reorderTab);
  const handleDrop = useCanvasStore(state => state.handleDrop);
  const setSplitRatio = useCanvasStore(state => state.setSplitRatio);
  const setSplitRatio2 = useCanvasStore(state => state.setSplitRatio2);
  const setActiveGroup = useCanvasStore(state => state.setActiveGroup);
  const setSplitMode = useCanvasStore(state => state.setSplitMode);
  const updateTabContent = useCanvasStore(state => state.updateTabContent);
  const setTabDirty = useCanvasStore(state => state.setTabDirty);
  const setTabFileDeletedFromDisk = useCanvasStore(state => state.setTabFileDeletedFromDisk);
  const setGrid9ColRatio = useCanvasStore(state => state.setGrid9ColRatio);
  const setGrid9RowRatio = useCanvasStore(state => state.setGrid9RowRatio);
  const applyGrid9Template = useCanvasStore(state => state.applyGrid9Template);
  const mergeGrid9Cells = useCanvasStore(state => state.mergeGrid9Cells);
  const removeGrid9Cell = useCanvasStore(state => state.removeGrid9Cell);

  const handleTabClick = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    switchToTab(tabId, groupId);
  }, [switchToTab]);

  const handleTabDoubleClick = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    promoteTab(tabId, groupId);
  }, [promoteTab]);

  const handleTabClose = useCallback((groupId: EditorGroupId) => async (tabId: string) => {
    if (onTabCloseWithDirtyCheck) {
      await onTabCloseWithDirtyCheck(tabId, groupId);
      return;
    }
    closeTab(tabId, groupId);
  }, [closeTab, onTabCloseWithDirtyCheck]);

  const handleCloseAllTabs = useCallback((groupId: EditorGroupId) => async () => {
    if (onTabCloseAllWithDirtyCheck) {
      await onTabCloseAllWithDirtyCheck(groupId);
      return;
    }
    closeAllTabs(groupId);
  }, [closeAllTabs, onTabCloseAllWithDirtyCheck]);

  const handleTabPin = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    togglePinTab(tabId, groupId);
  }, [togglePinTab]);

  const handleDragStart = useCallback((payload: TabDragPayload) => {
    startDrag(payload.tabId, payload.sourceGroupId);
  }, [startDrag]);

  const handleDragEnd = useCallback(() => {
    endDrag();
  }, [endDrag]);

  const handleReorderTab = useCallback((groupId: EditorGroupId) => (tabId: string, newIndex: number) => {
    reorderTab(tabId, groupId, newIndex);
  }, [reorderTab]);

  const handleDropOnGroup = useCallback((groupId: EditorGroupId) => (position: DropPosition) => {
    if (draggingTabId && draggingFromGroupId) {
      handleDrop(draggingTabId, draggingFromGroupId, groupId, position);
      endDrag();
    }
  }, [draggingTabId, draggingFromGroupId, handleDrop, endDrag]);

  const handleGroupFocus = useCallback((groupId: EditorGroupId) => () => {
    setActiveGroup(groupId);
  }, [setActiveGroup]);

  const handleContentChange = useCallback((groupId: EditorGroupId) => (tabId: string, content: PanelContent) => {
    updateTabContent(tabId, groupId, content);
  }, [updateTabContent]);

  const handleDirtyStateChange = useCallback((groupId: EditorGroupId) => (tabId: string, isDirty: boolean) => {
    setTabDirty(tabId, groupId, isDirty);
  }, [setTabDirty]);

  const handleTabFileDeletedFromDiskChange = useCallback(
    (groupId: EditorGroupId) => (tabId: string, missing: boolean) => {
      setTabFileDeletedFromDisk(tabId, groupId, missing);
    },
    [setTabFileDeletedFromDisk]
  );

  // Resident grid-template entry slot for the primary cell. Built here (not
  // inside the grid9 branch) so the TabBar grid-template toggle button stays
  // reachable in EVERY layout mode (none/h/v/grid) to enter grid9 — matches the
  // upstream resident grid9Slot at EditorArea's primary render. The slot's
  // active/toggle reflect the current splitMode (grid9 → exit, other → enter).
  const primaryGrid9Slot: Grid9Slot = grid9Slot ?? {
    active: layout.splitMode === 'grid9',
    onToggle: () => setSplitMode(layout.splitMode === 'grid9' ? 'none' : 'grid9'),
    label: 'gridTemplate.label',
    templates: [
      { cols: 2, rows: 2, label: 'gridTemplate.four' },
      { cols: 3, rows: 2, label: 'gridTemplate.six' },
      { cols: 3, rows: 3, label: 'gridTemplate.nine' },
      { cols: 4, rows: 4, label: 'gridTemplate.sixteen' },
    ],
    onApplyTemplate: (c, r) => applyGrid9Template(c, r),
  };

  const renderEditorGroup = (groupId: EditorGroupId, group: typeof primaryGroup, grid9Props?: Grid9CellProps) => (
    <EditorGroup
      groupId={groupId}
      group={group}
      isActive={activeGroupId === groupId}
      isSceneActive={isSceneActive}
      draggingTabId={draggingTabId}
      draggingFromGroupId={draggingFromGroupId}
      splitMode={layout.splitMode}
      workspacePath={workspacePath}
      onTabClick={handleTabClick(groupId)}
      onTabDoubleClick={handleTabDoubleClick(groupId)}
      onTabClose={handleTabClose(groupId)}
      onTabPin={handleTabPin(groupId)}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      onReorderTab={handleReorderTab(groupId)}
      onDrop={handleDropOnGroup(groupId)}
      onGroupFocus={handleGroupFocus(groupId)}
      onContentChange={handleContentChange(groupId)}
      onDirtyStateChange={handleDirtyStateChange(groupId)}
      onTabFileDeletedFromDiskChange={handleTabFileDeletedFromDiskChange(groupId)}
      onOpenMissionControl={groupId === 'primary' ? onOpenMissionControl : undefined}
      onCloseAllTabs={handleCloseAllTabs(groupId)}
      onInteraction={onInteraction}
      disablePopOut={disablePopOut}
      terminalResizeSuspended={terminalResizeSuspended}
      {...grid9Props}
      grid9Slot={grid9Props?.grid9Slot ?? (groupId === 'primary' ? primaryGrid9Slot : undefined)}
    />
  );

  const { splitMode, splitRatio, splitRatio2 } = layout;

  if (splitMode === 'grid9') {
    // Dynamic cols×rows grid (1..GRID_MAX_DIM each) that fully tiles the panel.
    // Only the activated rows/columns are rendered (no invisible outer frame), so
    // the template truly fills the panel edge to edge. Ratios are stored as
    // per-axis shares already normalized to sum to 1 (length === count).
    const cellTrack = (i: number) => 2 * i + 1;
    const handleTrack = (i: number) => 2 * i + 2;
    const cols = layout.grid9ColsCount;
    const rows = layout.grid9RowsCount;
    const gap = LAYOUT_CONFIG.RESIZER_WIDTH; // 4px resizer-track gaps
    const colRatios = Array.from({ length: cols }, (_, i) => layout.grid9ColRatios[i] ?? 1 / cols);
    const rowRatios = Array.from({ length: rows }, (_, i) => layout.grid9RowRatios[i] ?? 1 / rows);
    const gridTemplateColumns = colRatios.map(r => `${r}fr`).join(` ${gap}px `);
    const gridTemplateRows = rowRatios.map(r => `${r}fr`).join(` ${gap}px `);

    const nodes: React.ReactNode[] = [];
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const gid = EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c];
        const cell = layout.grid9Cells[gid] ?? createEditorGroupState();
        const isPrimary = r === 0 && c === 0;

        // Merge this cell into a neighbour: prefer the left cell in the same row,
        // otherwise the cell above. "Merge two small windows into one big window".
        let gridMerge: (() => void) | undefined;
        if (!isPrimary && cell.tabs.length > 0 && (c > 0 || r > 0)) {
          const target = c > 0
            ? EDITOR_GROUP_IDS[r * GRID_MAX_DIM + (c - 1)]
            : EDITOR_GROUP_IDS[(r - 1) * GRID_MAX_DIM + c];
          gridMerge = () => mergeGrid9Cells(gid, target);
        }
        const gridRemove = cell.tabs.length === 0 ? () => removeGrid9Cell(gid) : undefined;

        nodes.push(
          <div
            key={gid}
            data-bf-component="canvas-editor-area"
            data-bf-part="grid9Cell"
            data-bf-group={gid}
            data-bf-state={activeGroupId === gid ? 'active' : ''}
            className="canvas-editor-area__grid9-cell"
            style={{ gridColumn: cellTrack(c), gridRow: cellTrack(r) }}
          >
            {renderEditorGroup(gid, cell, {
              grid9Slot: isPrimary ? primaryGrid9Slot : undefined,
              gridMerge,
              gridRemove,
            })}
          </div>
        );

        // Column resizer after this cell (except last column): a vertical divider
        // that drags along clientX / container width.
        if (c < cols - 1) {
          nodes.push(
            <SplitHandle
              key={`${gid}-colh`}
              direction="horizontal"
              ratio={colRatios[c]}
              onRatioChange={(nr) => setGrid9ColRatio(c, nr)}
              containerRef={grid9Ref}
              minRatio={GRID9_RATIO_CONFIG.MIN}
              maxRatio={GRID9_RATIO_CONFIG.MAX}
              resetRatio={1 / cols}
              style={{ gridColumn: handleTrack(c), gridRow: cellTrack(r) }}
            />
          );
        }
      }
      // Row resizer after this row (except last row): a horizontal divider that
      // drags along clientY / container height, spanning all columns.
      if (r < rows - 1) {
        nodes.push(
          <SplitHandle
            key={`row-${r}`}
            direction="vertical"
            ratio={rowRatios[r]}
            onRatioChange={(nr) => setGrid9RowRatio(r, nr)}
            containerRef={grid9Ref}
            minRatio={GRID9_RATIO_CONFIG.MIN}
            maxRatio={GRID9_RATIO_CONFIG.MAX}
            resetRatio={1 / rows}
            style={{ gridColumn: '1 / -1', gridRow: handleTrack(r) }}
          />
        );
      }
    }

    return (
      <div
        data-bf-component="canvas-editor-area"
        data-bf-part="root"
        data-bf-layout="grid9"
        ref={grid9Ref}
        className="canvas-editor-area is-grid9"
      >
        <div
          className="canvas-editor-area__grid9-canvas"
          style={{ gridTemplateColumns, gridTemplateRows }}
        >
          {nodes}
        </div>
      </div>
    );
  }

  if (splitMode === 'none') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="none" ref={containerRef} className="canvas-editor-area">
        <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary">
          {renderEditorGroup('primary', primaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'horizontal') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="horizontal" ref={containerRef} className="canvas-editor-area is-split is-horizontal">
        <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary" style={{ width: `${splitRatio * 100}%` }}>
          {renderEditorGroup('primary', primaryGroup)}
        </div>
        <SplitHandle
          direction="horizontal"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div data-bf-component="canvas-editor-area" data-bf-part="secondary" className="canvas-editor-area__secondary" style={{ width: `${(1 - splitRatio) * 100}%` }}>
          {renderEditorGroup('secondary', secondaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'vertical') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="vertical" ref={containerRef} className="canvas-editor-area is-split is-vertical">
        <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary" style={{ height: `${splitRatio * 100}%` }}>
          {renderEditorGroup('primary', primaryGroup)}
        </div>
        <SplitHandle
          direction="vertical"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div data-bf-component="canvas-editor-area" data-bf-part="secondary" className="canvas-editor-area__secondary" style={{ height: `${(1 - splitRatio) * 100}%` }}>
          {renderEditorGroup('secondary', secondaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'grid') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="grid" ref={containerRef} className="canvas-editor-area is-grid">
        <div data-bf-component="canvas-editor-area" data-bf-part="topRow" ref={topRowRef} className="canvas-editor-area__top-row" style={{ flex: `0 0 calc(${splitRatio * 100}% - 2px)` }}>
          <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary" style={{ flex: `0 0 calc(${splitRatio2 * 100}% - 2px)` }}>
            {renderEditorGroup('primary', primaryGroup)}
          </div>
          <SplitHandle
            direction="horizontal"
            ratio={splitRatio2}
            onRatioChange={setSplitRatio2}
            containerRef={topRowRef}
          />
          <div data-bf-component="canvas-editor-area" data-bf-part="secondary" className="canvas-editor-area__secondary" style={{ flex: 1, minWidth: 0 }}>
            {renderEditorGroup('secondary', secondaryGroup)}
          </div>
        </div>
        <SplitHandle
          direction="vertical"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div data-bf-component="canvas-editor-area" data-bf-part="tertiary" className="canvas-editor-area__tertiary" style={{ flex: 1, minHeight: 0 }}>
          {renderEditorGroup('tertiary', tertiaryGroup)}
        </div>
      </div>
    );
  }

  return null;
};

EditorArea.displayName = 'EditorArea';

export default EditorArea;

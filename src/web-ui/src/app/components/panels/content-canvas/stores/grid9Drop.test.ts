/**
 * @vitest-environment jsdom
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import type { EditorGroupId } from '@/app/components/panels/content-canvas/types';

function tabsIn(groupId: string): { title: string; id: string }[] {
  const state = useAgentCanvasStore.getState();
  if (state.layout.splitMode === 'grid9') {
    return (state.layout.grid9Cells[groupId as EditorGroupId]?.tabs ?? []) as { title: string; id: string }[];
  }
  if (groupId === 'primary') return state.primaryGroup.tabs as { title: string; id: string }[];
  if (groupId === 'secondary') return state.secondaryGroup.tabs as { title: string; id: string }[];
  return state.tertiaryGroup.tabs as { title: string; id: string }[];
}

function findTab(groupId: string, title: string) {
  return tabsIn(groupId).find(t => t.title === title);
}

const md = (title: string) => ({ type: 'markdown-viewer', title, data: {} });

describe('grid9 drag-drop: independent rows/columns', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('moves a tab from primary to slot6 when dropped in grid9 mode (center)', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.addTab(md('B'), 'active', 'primary');
    store.setSplitMode('grid9');
    const tabB = findTab('primary', 'B');
    expect(tabB).toBeDefined();

    useAgentCanvasStore.getState().handleDrop(tabB!.id, 'primary', 'slot6', 'center');
    const after = useAgentCanvasStore.getState();
    expect(tabsIn('slot6').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('primary').some(t => t.title === 'B')).toBe(false);
    // center drop into slot6 (row1 col1 in 4x4 row-major) grows rows to 2 and cols to 2.
    expect(after.layout.grid9RowsCount).toBe(2);
    expect(after.layout.grid9ColsCount).toBe(2);
  });

  it('keeps grid9 mode when closing a tab', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');
    const tabA = findTab('primary', 'A')!;
    useAgentCanvasStore.getState().closeTab(tabA.id, 'primary');
    expect(useAgentCanvasStore.getState().layout.splitMode).toBe('grid9');
  });

  it('none-mode center drop does NOT jump to grid9 (single-column mode preserved)', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.addTab(md('B'), 'active', 'primary');
    expect(useAgentCanvasStore.getState().layout.splitMode).toBe('none');

    const tabB = findTab('primary', 'B')!;
    useAgentCanvasStore.getState().handleDrop(tabB.id, 'primary', 'primary', 'center');

    expect(useAgentCanvasStore.getState().layout.splitMode).toBe('none');
  });

  it('drag-natural upgrade: edge drop in none mode still enters horizontal split', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.addTab(md('B'), 'active', 'primary');
    const tabB = findTab('primary', 'B')!;
    useAgentCanvasStore.getState().handleDrop(tabB.id, 'primary', 'primary', 'right');

    const after = useAgentCanvasStore.getState();
    expect(after.layout.splitMode).toBe('horizontal');
    expect(after.secondaryGroup.tabs.some(t => t.title === 'B')).toBe(true);
  });

  it('rows-first: bottom edge drops grow rows independently (1→2→3 rows)', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(1);
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(1);

    const a = findTab('primary', 'A')!;
    useAgentCanvasStore.getState().handleDrop(a.id, 'primary', 'primary', 'bottom');
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(2);
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(1);

    const a2 = tabsIn(useAgentCanvasStore.getState().activeGroupId).find(t => t.title === 'A')!;
    useAgentCanvasStore.getState().handleDrop(a2.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'bottom');
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(3);
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(1);
  });

  it('columns-first: right edge drops grow columns independently (1→2→3 cols)', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');

    const a = findTab('primary', 'A')!;
    useAgentCanvasStore.getState().handleDrop(a.id, 'primary', 'primary', 'right');
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(2);
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(1);

    const a2 = tabsIn(useAgentCanvasStore.getState().activeGroupId).find(t => t.title === 'A')!;
    useAgentCanvasStore.getState().handleDrop(a2.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'right');
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(3);
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(1);
  });

  it('grows columns to 4 in grid9 mode (4x4)', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');

    let tab = findTab('primary', 'A')!;
    for (let expected = 2; expected <= 4; expected++) {
      useAgentCanvasStore.getState().handleDrop(tab.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'right');
      expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(expected);
      expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(1);
      tab = tabsIn(useAgentCanvasStore.getState().activeGroupId).find(t => t.title === 'A')!;
    }
    // 4 is the max: another right drop keeps 4 columns.
    useAgentCanvasStore.getState().handleDrop(tab.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'right');
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(4);
  });

  it('grows rows to 4 in grid9 mode (4x4)', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');

    let tab = findTab('primary', 'A')!;
    for (let expected = 2; expected <= 4; expected++) {
      useAgentCanvasStore.getState().handleDrop(tab.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'bottom');
      expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(expected);
      expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(1);
      tab = tabsIn(useAgentCanvasStore.getState().activeGroupId).find(t => t.title === 'A')!;
    }
    useAgentCanvasStore.getState().handleDrop(tab.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'bottom');
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(4);
  });

  it('center drop into a row3/col3 slot grows the grid to 4x4', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');
    // slot16 = row 3, col 3 (4x4 row-major).
    const a = findTab('primary', 'A')!;
    useAgentCanvasStore.getState().handleDrop(a.id, 'primary', 'slot16', 'center');
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(4);
    expect(s.layout.grid9RowsCount).toBe(4);
    expect(tabsIn('slot16').some(t => t.title === 'A')).toBe(true);
  });

  it('rows-then-columns: bottom then right builds a 2x2 grid in any order', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');

    const a = findTab('primary', 'A')!;
    useAgentCanvasStore.getState().handleDrop(a.id, 'primary', 'primary', 'bottom');
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(2);
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(1);

    const a2 = tabsIn(useAgentCanvasStore.getState().activeGroupId).find(t => t.title === 'A')!;
    useAgentCanvasStore.getState().handleDrop(a2.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'right');
    const after = useAgentCanvasStore.getState();
    expect(after.layout.grid9ColsCount).toBe(2);
    expect(after.layout.grid9RowsCount).toBe(2);
  });

  it('columns-then-rows: right then bottom also builds a 2x2 grid', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');

    const a = findTab('primary', 'A')!;
    useAgentCanvasStore.getState().handleDrop(a.id, 'primary', 'primary', 'right');
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(2);
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(1);

    const a2 = tabsIn(useAgentCanvasStore.getState().activeGroupId).find(t => t.title === 'A')!;
    useAgentCanvasStore.getState().handleDrop(a2.id, useAgentCanvasStore.getState().activeGroupId, useAgentCanvasStore.getState().activeGroupId, 'bottom');
    const after = useAgentCanvasStore.getState();
    expect(after.layout.grid9ColsCount).toBe(2);
    expect(after.layout.grid9RowsCount).toBe(2);
  });

  it('closing the last tab in a trailing row shrinks the row count', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.setSplitMode('grid9');

    const a = findTab('primary', 'A')!;
    useAgentCanvasStore.getState().handleDrop(a.id, 'primary', 'primary', 'bottom');
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(2);
    expect(tabsIn('slot5').some(t => t.title === 'A')).toBe(true);

    const tab5 = findTab('slot5', 'A')!;
    useAgentCanvasStore.getState().closeTab(tab5.id, 'slot5');
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(1);
  });

  it('grid(3-pane) expands to grid9 by dropping below the bottom pane', () => {
    const store = useAgentCanvasStore.getState();
    store.addTab(md('A'), 'active', 'primary');
    store.addTab(md('B'), 'active', 'primary');
    // Reach 2-pane: none → horizontal (right).
    const tabB = findTab('primary', 'B')!;
    useAgentCanvasStore.getState().handleDrop(tabB.id, 'primary', 'primary', 'right');
    expect(useAgentCanvasStore.getState().layout.splitMode).toBe('horizontal');

    // Reach 3-pane: a fresh tab dropped to the bottom grows the grid.
    store.addTab(md('C'), 'active', 'primary');
    const tabC = findTab('primary', 'C')!;
    useAgentCanvasStore.getState().handleDrop(tabC.id, 'primary', 'tertiary', 'bottom');
    expect(useAgentCanvasStore.getState().layout.splitMode).toBe('grid');
    expect(tabsIn('tertiary').some(t => t.title === 'C')).toBe(true);

    // Expand into grid9 by dropping below tertiary → rows=2, cols=2.
    const tabC2 = findTab('tertiary', 'C')!;
    useAgentCanvasStore.getState().handleDrop(tabC2.id, 'tertiary', 'tertiary', 'bottom');
    const after = useAgentCanvasStore.getState();
    expect(after.layout.splitMode).toBe('grid9');
    expect(after.layout.grid9ColsCount).toBe(2);
    expect(after.layout.grid9RowsCount).toBe(2);
    // slot6 = row1 col1 in 4x4 row-major — the cell directly below tertiary
    // (row0 col2), which is what the grid→grid9 upgrade path means by
    // "dropping below the bottom pane".
    expect(tabsIn('slot6').some(t => t.title === 'C')).toBe(true);
  });
});

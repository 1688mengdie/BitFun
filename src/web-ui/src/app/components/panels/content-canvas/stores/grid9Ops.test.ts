/**
 * @vitest-environment jsdom
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import type { EditorGroupId } from '@/app/components/panels/content-canvas/types';

/**
 * In grid9 mode tabs live in `layout.grid9Cells[gid]`; in none/h/v/grid mode
 * they live in the three legacy groups. This helper reads whichever is active.
 */
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

function addTab(title: string, groupId: string) {
  useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title, data: {} }, 'active', groupId as EditorGroupId);
}

const sum = (arr: number[]) => arr.reduce((a, b) => a + b, 0);

describe('grid9 templates', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('applyGrid9Template 2x2 sets cols/rows and splitMode', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.splitMode).toBe('grid9');
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
  });

  it('applyGrid9Template clamps to 1..GRID_MAX_DIM', () => {
    useAgentCanvasStore.getState().applyGrid9Template(9, 0);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(4);
    expect(s.layout.grid9RowsCount).toBe(1);
  });

  it('applyGrid9Template supports 4x4 and clamps beyond to 4', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.splitMode).toBe('grid9');
    expect(s.layout.grid9ColsCount).toBe(4);
    expect(s.layout.grid9RowsCount).toBe(4);
    useAgentCanvasStore.getState().applyGrid9Template(7, 9);
    const s2 = useAgentCanvasStore.getState();
    expect(s2.layout.grid9ColsCount).toBe(4);
    expect(s2.layout.grid9RowsCount).toBe(4);
  });

  it('4x4 template keeps tabs in a slot inside the template', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    addTab('A', 'slot15'); // row3 col3 — inside a 4x4 template
    expect(tabsIn('slot15').some(t => t.title === 'A')).toBe(true);
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(4);
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(4);
  });

  it('applyGrid9Template moves tabs outside the template into primary (no silent drop)', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    addTab('C', 'tertiary'); // row0 col2 — outside a 2x2 template
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    const s = useAgentCanvasStore.getState();
    expect(tabsIn('primary').some(t => t.title === 'C')).toBe(true);
    expect(tabsIn('tertiary').length).toBe(0);
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
  });

  it('applyGrid9Template resets ratios to equal shares (axis sums to 1)', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    useAgentCanvasStore.getState().setGrid9ColRatio(0, 0.6);
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColRatios).toHaveLength(3);
    expect(s.layout.grid9RowRatios).toHaveLength(3);
    expect(sum(s.layout.grid9ColRatios)).toBeCloseTo(1);
    expect(sum(s.layout.grid9RowRatios)).toBeCloseTo(1);
    expect(s.layout.grid9ColRatios[0]).toBeCloseTo(1 / 3);
    expect(s.layout.grid9ColRatios[1]).toBeCloseTo(1 / 3);
    expect(s.layout.grid9RowRatios[0]).toBeCloseTo(1 / 3);
  });

  it('setGrid9ColRatio renormalizes the column axis to sum to 1 and leaves rows untouched', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 2);
    useAgentCanvasStore.getState().setGrid9ColRatio(0, 0.6);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColRatios[0]).toBeCloseTo(0.6);
    expect(sum(s.layout.grid9ColRatios)).toBeCloseTo(1);
    // Rows stay at equal shares (2 rows -> 0.5 each).
    expect(s.layout.grid9RowRatios).toHaveLength(2);
    expect(sum(s.layout.grid9RowRatios)).toBeCloseTo(1);
    expect(s.layout.grid9RowRatios[0]).toBeCloseTo(0.5);
  });

  it('growing the other axis (adding a row) does NOT change column ratios', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 1);
    addTab('A', 'primary'); // col0 row0
    addTab('B', 'secondary'); // col1 row0 — keeps the trailing column non-empty
    useAgentCanvasStore.getState().setGrid9ColRatio(0, 0.7);
    const store = useAgentCanvasStore.getState();
    // Grow a row below primary; the boundary grid must stay 2 columns wide so no
    // trailing-column collapse fires and the column ratios are preserved exactly.
    store.handleDrop(findTab('primary', 'A')!.id, 'primary', 'primary', 'bottom');
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9ColRatios[0]).toBeCloseTo(0.7);
    expect(sum(s.layout.grid9ColRatios)).toBeCloseTo(1);
  });

  it('growing the column axis preserves the relative proportions of existing ratios', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 1);
    addTab('A', 'primary');
    useAgentCanvasStore.getState().setGrid9ColRatio(0, 0.7); // col1 -> 0.3
    const store = useAgentCanvasStore.getState();
    store.handleDrop(findTab('primary', 'A')!.id, 'primary', 'primary', 'right');
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColRatios).toHaveLength(3);
    expect(sum(s.layout.grid9ColRatios)).toBeCloseTo(1);
    // 0.7 : 0.3 relative proportion preserved after appending the new share.
    expect(s.layout.grid9ColRatios[0] / s.layout.grid9ColRatios[1]).toBeCloseTo(0.7 / 0.3);
  });

  it('applyGrid9Template resets activeGroupId to primary when it points outside', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    addTab('A', 'slot7'); // row1 col2 in 4x4 row-major — outside a 2x2 template
    useAgentCanvasStore.getState().setActiveGroup('slot7');
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('primary');
  });

  it('applyGrid9Template keeps activeGroupId inside the template', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    addTab('A', 'secondary');
    useAgentCanvasStore.getState().setActiveGroup('secondary');
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('secondary');
  });
});

describe('grid -> grid9 upgrade (existing boundary)', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('keeps pre-existing tertiary tabs in tertiary (outside 2x2 template, preserved not dropped)', () => {
    const store = useAgentCanvasStore.getState();
    store.setSplitMode('grid');
    store.addTab({ type: 'markdown-viewer', title: 'T', data: {} }, 'active', 'tertiary');
    store.addTab({ type: 'markdown-viewer', title: 'D', data: {} }, 'active', 'primary');
    const dragged = tabsIn('primary').find(t => t.title === 'D')!;
    // Drag D onto the bottom edge of tertiary: the grid→grid9 upgrade path
    // lands D in slot6 (row1 col1), the cell below tertiary.
    store.handleDrop(dragged.id, 'primary', 'tertiary', 'bottom');
    const s = useAgentCanvasStore.getState();
    expect(s.layout.splitMode).toBe('grid9');
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(tabsIn('slot6').some(t => t.title === 'D')).toBe(true);
    // Pre-existing tertiary tab T is never silently dropped (row0 col2 is
    // outside the 2x2 template so it is preserved but not rendered).
    expect(tabsIn('tertiary').some(t => t.title === 'T')).toBe(true);
  });
});

describe('mergeGrid9Cells', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('merges tabs from secondary into primary and empties secondary', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    useAgentCanvasStore.getState().mergeGrid9Cells('secondary', 'primary');
    const s = useAgentCanvasStore.getState();
    expect(tabsIn('primary').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('secondary').length).toBe(0);
    expect(s.activeGroupId).toBe('primary');
  });

  it('no-op when source is empty or same group', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    const before = tabsIn('primary').length;
    useAgentCanvasStore.getState().mergeGrid9Cells('secondary', 'primary');
    expect(tabsIn('primary').length).toBe(before);
    useAgentCanvasStore.getState().mergeGrid9Cells('primary', 'primary');
    expect(tabsIn('primary').length).toBe(before);
  });

  it('merges active tab id from source into target', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    const tabB = findTab('secondary', 'B');
    useAgentCanvasStore.getState().switchToTab(tabB!.id, 'secondary');
    useAgentCanvasStore.getState().mergeGrid9Cells('secondary', 'primary');
    const s = useAgentCanvasStore.getState();
    expect(tabsIn('primary').some(t => t.title === 'B')).toBe(true);
    expect(s.layout.grid9Cells['primary']?.activeTabId).toBe(tabB!.id);
  });
});

describe('removeGrid9Cell', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('removing a blank middle column shifts columns left and keeps tabs', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 2);
    addTab('A', 'primary');
    addTab('B', 'tertiary'); // row0 col2
    useAgentCanvasStore.getState().removeGrid9Cell('secondary'); // row0 col1
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(tabsIn('secondary').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('tertiary').length).toBe(0);
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
  });

  it('removing a blank column renormalizes the column ratios to sum to 1', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 2);
    addTab('A', 'primary');
    addTab('B', 'tertiary');
    useAgentCanvasStore.getState().setGrid9ColRatio(2, 0.6);
    useAgentCanvasStore.getState().removeGrid9Cell('secondary');
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9ColRatios).toHaveLength(2);
    expect(sum(s.layout.grid9ColRatios)).toBeCloseTo(1);
  });

  it('removing the first column shifts everything left without losing tabs', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    useAgentCanvasStore.getState().removeGrid9Cell('primary');
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
    expect(tabsIn('primary').some(t => t.title === 'B')).toBe(true);
    expect(s.activeGroupId).toBe('primary');
  });

  it('removing a blank row shifts rows up', () => {
    useAgentCanvasStore.getState().applyGrid9Template(1, 3);
    addTab('A', 'primary');
    addTab('B', 'slot9'); // row2 col0 in 4x4 row-major
    useAgentCanvasStore.getState().removeGrid9Cell('slot5'); // row1 col0
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(tabsIn('slot5').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('slot9').length).toBe(0);
  });

  it('removing a blank middle column on a 4x4 grid shifts columns and keeps tabs', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    addTab('A', 'primary');
    addTab('B', 'tertiary'); // row0 col2
    useAgentCanvasStore.getState().removeGrid9Cell('secondary'); // row0 col1
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(3);
    expect(s.layout.grid9RowsCount).toBe(4);
    expect(tabsIn('secondary').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('tertiary').length).toBe(0);
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
  });

  it('removing a blank row on a 4-row grid shifts rows up', () => {
    useAgentCanvasStore.getState().applyGrid9Template(1, 4);
    addTab('A', 'primary');
    addTab('B', 'slot13'); // row3 col0 in 4x4 row-major
    useAgentCanvasStore.getState().removeGrid9Cell('slot5'); // row1 col0
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(3);
    expect(tabsIn('slot9').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('slot13').length).toBe(0);
  });

  it('does nothing on a 1x1 grid', () => {
    useAgentCanvasStore.getState().applyGrid9Template(1, 1);
    addTab('A', 'primary');
    useAgentCanvasStore.getState().removeGrid9Cell('primary');
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(1);
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
  });

  it('fixes activeGroupId when the active cell is removed', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'secondary');
    useAgentCanvasStore.getState().setActiveGroup('secondary');
    useAgentCanvasStore.getState().removeGrid9Cell('secondary');
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('primary');
  });
});

describe('enterGrid9', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('enters grid9 with clamped counts and preserves primary content', () => {
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'A', data: {} }, 'active', 'primary');
    useAgentCanvasStore.getState().enterGrid9(2, 2);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.splitMode).toBe('grid9');
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
  });
});

describe('closeAllTabs (no-arg) clears all grid9 slots', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('empties every group slot (primary..slot16) while keeping pinned tabs', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    const seed: string[] = [
      'primary', 'secondary', 'tertiary',
      'slot4', 'slot5', 'slot6', 'slot7', 'slot8', 'slot9',
      'slot10', 'slot11', 'slot12', 'slot13', 'slot14', 'slot15', 'slot16',
    ];
    seed.forEach((gid, i) => addTab(`tab-${i}`, gid));
    seed.forEach(gid => expect(tabsIn(gid).length).toBe(1));

    useAgentCanvasStore.getState().closeAllTabs();

    seed.forEach(gid => expect(tabsIn(gid).length).toBe(0));
  });

  it('keeps pinned tabs from every group, not only slots 4-9', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'P10', data: {} }, 'pinned', 'slot10');
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'U10', data: {} }, 'preview', 'slot10');
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'P16', data: {} }, 'pinned', 'slot16');
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'U16', data: {} }, 'preview', 'slot16');

    useAgentCanvasStore.getState().closeAllTabs();

    expect(tabsIn('slot10').some(t => t.title === 'U10')).toBe(false);
    expect(tabsIn('slot16').some(t => t.title === 'U16')).toBe(false);
    expect(tabsIn('primary').some(t => t.title === 'P10')).toBe(true);
    expect(tabsIn('primary').some(t => t.title === 'P16')).toBe(true);
    expect(useAgentCanvasStore.getState().layout.splitMode).toBe('none');
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('primary');
  });
});

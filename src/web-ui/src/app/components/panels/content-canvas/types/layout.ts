/**
 * Layout-related type definitions.
 * ContentCanvas layout system.
 */

import type { EditorGroupState } from './tab';

/**
 * Split modes
 * - none: single column
 * - horizontal: left/right
 * - vertical: top/bottom
 * - grid: top left/right + bottom (T layout)
 * - grid9: free-form 4x4 grid (up to 16 cells, drag/merge/remove)
 */
export type SplitMode = 'none' | 'horizontal' | 'vertical' | 'grid' | 'grid9';

export type AnchorPosition = 'bottom' | 'right' | 'hidden';

export type DropPosition = 'left' | 'right' | 'top' | 'bottom' | 'center';

/** Max grid dimension: the grid9 canvas is at most 4 columns × 4 rows (16 cells). */
export const GRID_MAX_DIM = 4;

/**
 * Editor group ids. The first three canonical ids are the legacy layout groups
 * (external consumers hard-code 'primary'/'secondary'/'tertiary'); slot4..slot16
 * extend the canvas to a full 4x4 grid (row-major).
 */
export type EditorGroupId =
  | 'primary'
  | 'secondary'
  | 'tertiary'
  | 'slot4'
  | 'slot5'
  | 'slot6'
  | 'slot7'
  | 'slot8'
  | 'slot9'
  | 'slot10'
  | 'slot11'
  | 'slot12'
  | 'slot13'
  | 'slot14'
  | 'slot15'
  | 'slot16';

/** All editor group ids in grid order (row-major 4x4). */
export const EDITOR_GROUP_IDS: readonly EditorGroupId[] = [
  'primary',
  'secondary',
  'tertiary',
  'slot4',
  'slot5',
  'slot6',
  'slot7',
  'slot8',
  'slot9',
  'slot10',
  'slot11',
  'slot12',
  'slot13',
  'slot14',
  'slot15',
  'slot16',
] as const;

/** Column index (0..3) of each group in the 4x4 grid. */
export const EDITOR_GROUP_COL: Record<EditorGroupId, number> = {
  primary: 0,
  secondary: 1,
  tertiary: 2,
  slot4: 3,
  slot5: 0,
  slot6: 1,
  slot7: 2,
  slot8: 3,
  slot9: 0,
  slot10: 1,
  slot11: 2,
  slot12: 3,
  slot13: 0,
  slot14: 1,
  slot15: 2,
  slot16: 3,
};

/** Row index (0..3) of each group in the 4x4 grid. */
export const EDITOR_GROUP_ROW: Record<EditorGroupId, number> = {
  primary: 0,
  secondary: 0,
  tertiary: 0,
  slot4: 0,
  slot5: 1,
  slot6: 1,
  slot7: 1,
  slot8: 1,
  slot9: 2,
  slot10: 2,
  slot11: 2,
  slot12: 2,
  slot13: 3,
  slot14: 3,
  slot15: 3,
  slot16: 3,
};

export interface LayoutState {
  splitMode: SplitMode;
  /** Primary split ratio: left/top in 2-pane; top/bottom or left/right in 3-pane */
  splitRatio: number;
  /** Secondary split ratio: grid-top left/right or grid-bottom left/right */
  splitRatio2: number;
  /**
   * grid9 cells: active grid pane states keyed by editor group id. Populated only
   * in grid9 mode; the three legacy fields stay the single source of truth for
   * none/h/v/grid and are left untouched while the layout is grid9.
   */
  grid9Cells: Partial<Record<EditorGroupId, EditorGroupState>>;
  /** Activated column count in grid9 mode (1..GRID_MAX_DIM). */
  grid9ColsCount: number;
  /** Activated row count in grid9 mode (1..GRID_MAX_DIM). */
  grid9RowsCount: number;
  /** Column ratios (relative shares summing to 1); length === grid9ColsCount. */
  grid9ColRatios: number[];
  /** Row ratios (relative shares summing to 1); length === grid9RowsCount. */
  grid9RowRatios: number[];
  anchorPosition: AnchorPosition;
  anchorSize: number;
  isMaximized: boolean;
}

export interface CanvasState {
  primaryGroup: EditorGroupState;
  secondaryGroup: EditorGroupState;
  tertiaryGroup: EditorGroupState;
  activeGroupId: EditorGroupId;
  layout: LayoutState;
  isMissionControlOpen: boolean;
}

export interface CanvasPersistState {
  primaryGroup: EditorGroupState;
  secondaryGroup: EditorGroupState;
  tertiaryGroup: EditorGroupState;
  activeGroupId: EditorGroupId;
  layout: LayoutState;
}

/**
 * Layout configuration constants.
 */
export const LAYOUT_CONFIG = {
  /** Min split ratio */
  MIN_SPLIT_RATIO: 0.2,
  /** Max split ratio */
  MAX_SPLIT_RATIO: 0.8,
  /** Default split ratio */
  DEFAULT_SPLIT_RATIO: 0.5,
  /** Min anchor size */
  MIN_ANCHOR_SIZE: 100,
  /** Max anchor size */
  MAX_ANCHOR_SIZE: 500,
  /** Default anchor size */
  DEFAULT_ANCHOR_SIZE: 200,
  /** Resizer width */
  RESIZER_WIDTH: 4,
  /** Snap range */
  SNAP_RANGE: 15,
  /** Transition duration */
  TRANSITION_DURATION: 200,
} as const;

/**
 * Create initial editor group state.
 */
export const createEditorGroupState = (): EditorGroupState => ({
  tabs: [],
  activeTabId: null,
});

export const createLayoutState = (): LayoutState => ({
  splitMode: 'none',
  splitRatio: LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO,
  splitRatio2: LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO,
  grid9Cells: {},
  grid9ColsCount: 1,
  grid9RowsCount: 1,
  grid9ColRatios: [1],
  grid9RowRatios: [1],
  anchorPosition: 'hidden',
  anchorSize: LAYOUT_CONFIG.DEFAULT_ANCHOR_SIZE,
  isMaximized: false,
});

export const createCanvasState = (): CanvasState => ({
  primaryGroup: createEditorGroupState(),
  secondaryGroup: createEditorGroupState(),
  tertiaryGroup: createEditorGroupState(),
  activeGroupId: 'primary',
  layout: createLayoutState(),
  isMissionControlOpen: false,
});

/**
 * Clamp split ratio to valid range.
 */
export const clampSplitRatio = (ratio: number): number => {
  return Math.max(
    LAYOUT_CONFIG.MIN_SPLIT_RATIO,
    Math.min(LAYOUT_CONFIG.MAX_SPLIT_RATIO, ratio)
  );
};

/**
 * Clamp anchor size to valid range.
 */
export const clampAnchorSize = (size: number): number => {
  return Math.max(
    LAYOUT_CONFIG.MIN_ANCHOR_SIZE,
    Math.min(LAYOUT_CONFIG.MAX_ANCHOR_SIZE, size)
  );
};

/**
 * Grid9 column/row ratio bounds.
 *
 * Equal bounds for split ratios and grid9 ratios (MIN 0.2 / MAX 0.8) so a
 * dragged split never reports a ratio the store later clamps to a different
 * window. grid9 stores per-axis shares normalized to 1.0 at write time.
 */
export const GRID9_RATIO_CONFIG = {
  MIN: 0.2,
  MAX: 0.8,
} as const;

/**
 * Clamp a single grid9 column/row ratio. Ratios are relative shares of the
 * container along that axis; two adjacent resizers can both reach the max.
 */
export const clampGrid9Ratio = (ratio: number): number => {
  return Math.max(
    GRID9_RATIO_CONFIG.MIN,
    Math.min(GRID9_RATIO_CONFIG.MAX, ratio)
  );
};

/**
 * A grid-template preset: a cols×rows arrangement plus a display label.
 * The label is an i18n key resolved by the consumer (the TabBar menu owns
 * the actual copy; this type only carries the geometry + a label handle).
 */
export interface Grid9Template {
  cols: number;
  rows: number;
  label: string;
}

/**
 * grid9 slot info threaded down to the TabBar (primary group only) so it can
 * render a grid-template toggle/menu. The object is built by EditorArea and
 * passed through ContentCanvas → EditorArea → EditorGroup → TabBar. C3 only
 * threads the type; C4 renders the menu.
 */
export interface Grid9Slot {
  /** Whether grid9 mode is currently active. */
  active: boolean;
  /** Toggle grid9 mode on/off. */
  onToggle: () => void;
  /** Renderable label for the template toggle button (i18n key). */
  label: string;
  /** Preset templates shown in the dropdown. */
  templates?: Grid9Template[];
  /** Apply a preset template (cols × rows). */
  onApplyTemplate?: (cols: number, rows: number) => void;
}

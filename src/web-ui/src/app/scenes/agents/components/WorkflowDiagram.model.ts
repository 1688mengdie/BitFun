/**
 * Pure layout model for WorkflowDiagram — kept separate from the component
 * file so the component only exports React components (react-refresh
 * contract).
 *
 * Semantics taken from the DSH (dsh-agent-teams) reference panel:
 *  - `compactDagLayout` = dependency-depth columns (lanes), left → right;
 *    each node sits in the column of its longest dependency chain.
 *  - Edges are cubic bezier SVG paths from a source node's right edge to a
 *    target node's left edge (ActivityPanel.tsx:395-400, activity-model.ts:207).
 */

export interface WorkflowDiagramNode {
  id: string;
  label: string;
  description?: string;
  agent?: string;
  gate?: boolean;
}

export interface WorkflowDiagramEdge {
  from: string;
  to: string;
  label?: string;
  condition?: string;
}

export interface WorkflowDiagramLayoutNode {
  node: WorkflowDiagramNode;
  x: number;
  y: number;
}

export interface WorkflowDiagramLayoutEdge {
  from: string;
  to: string;
  label?: string;
  path: string;
}

export interface WorkflowDiagramLayout {
  width: number;
  height: number;
  nodes: WorkflowDiagramLayoutNode[];
  edges: WorkflowDiagramLayoutEdge[];
}

export const WORKFLOW_NODE_WIDTH = 172;
export const WORKFLOW_NODE_HEIGHT = 46;
export const WORKFLOW_COLUMN_GAP = 64;
export const WORKFLOW_ROW_GAP = 48;
export const WORKFLOW_PADDING = 20;
/** Bezier curve control offset from each edge endpoint (≈ half the column gap). */
const WORKFLOW_EDGE_CURVE = 32;

/**
 * Depth = the length of the longest dependency chain ending at a node
 * (0 for roots). Cycle-safe: a dependency already being visited contributes
 * 0 so malformed cyclic presets cannot recurse forever.
 */
export function computeWorkflowDepths(
  nodes: ReadonlyArray<WorkflowDiagramNode>,
  edges: ReadonlyArray<WorkflowDiagramEdge>,
): Map<string, number> {
  const nodeIds = new Set(nodes.map((node) => node.id));
  const dependencies = new Map<string, string[]>();
  for (const edge of edges) {
    if (!nodeIds.has(edge.from) || !nodeIds.has(edge.to)) continue;
    const list = dependencies.get(edge.to) ?? [];
    if (!list.includes(edge.from)) list.push(edge.from);
    dependencies.set(edge.to, list);
  }

  const depth = new Map<string, number>();
  const visiting = new Set<string>();

  const resolve = (id: string): number => {
    const cached = depth.get(id);
    if (cached !== undefined) return cached;
    if (visiting.has(id)) return 0; // cycle guard
    visiting.add(id);
    let longest = 0;
    for (const dep of dependencies.get(id) ?? []) {
      longest = Math.max(longest, resolve(dep) + 1);
    }
    visiting.delete(id);
    depth.set(id, longest);
    return longest;
  };

  for (const node of nodes) resolve(node.id);
  return depth;
}

/**
 * Produce the DSH-style compact depth-column layout. Columns are dependency
 * depths (stable by node id order inside a column); rows are index order.
 */
export function computeWorkflowDagLayout(
  nodes: ReadonlyArray<WorkflowDiagramNode>,
  edges: ReadonlyArray<WorkflowDiagramEdge>,
): WorkflowDiagramLayout {
  const depth = computeWorkflowDepths(nodes, edges);

  const byDepth = new Map<number, WorkflowDiagramNode[]>();
  for (const node of nodes) {
    const column = Math.max(0, depth.get(node.id) ?? 0);
    const stage = byDepth.get(column) ?? [];
    stage.push(node);
    byDepth.set(column, stage);
  }
  const columns = [...byDepth.entries()].sort(([left], [right]) => left - right);

  const positions = new Map<string, { x: number; y: number }>();
  const layoutNodes: WorkflowDiagramLayoutNode[] = [];
  let maxRows = 0;

  for (const [columnIndex, [, stage]] of columns.entries()) {
    const x = WORKFLOW_PADDING + columnIndex * (WORKFLOW_NODE_WIDTH + WORKFLOW_COLUMN_GAP);
    let row = 0;
    for (const node of stage) {
      const y = WORKFLOW_PADDING + row * (WORKFLOW_NODE_HEIGHT + WORKFLOW_ROW_GAP);
      positions.set(node.id, { x, y });
      layoutNodes.push({ node, x, y });
      row += 1;
    }
    maxRows = Math.max(maxRows, row);
  }

  const columnCount = columns.length;
  const width = columnCount === 0
    ? 0
    : WORKFLOW_PADDING * 2 + columnCount * WORKFLOW_NODE_WIDTH
      + (columnCount - 1) * WORKFLOW_COLUMN_GAP;
  const height = columnCount === 0
    ? 0
    : WORKFLOW_PADDING * 2 + maxRows * WORKFLOW_NODE_HEIGHT
      + (maxRows - 1) * WORKFLOW_ROW_GAP;

  const layoutEdges: WorkflowDiagramLayoutEdge[] = [];
  for (const edge of edges) {
    const source = positions.get(edge.from);
    const target = positions.get(edge.to);
    if (!source || !target) continue;
    const x1 = source.x + WORKFLOW_NODE_WIDTH;
    const y1 = source.y + WORKFLOW_NODE_HEIGHT / 2;
    const x2 = target.x;
    const y2 = target.y + WORKFLOW_NODE_HEIGHT / 2;
    layoutEdges.push({
      from: edge.from,
      to: edge.to,
      label: edge.label ?? edge.condition,
      path: `M${x1} ${y1}C${x1 + WORKFLOW_EDGE_CURVE} ${y1},${x2 - WORKFLOW_EDGE_CURVE} ${y2},${x2} ${y2}`,
    });
  }

  return { width, height, nodes: layoutNodes, edges: layoutEdges };
}

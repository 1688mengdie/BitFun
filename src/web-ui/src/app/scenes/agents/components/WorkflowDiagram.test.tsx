// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import WorkflowDiagram from './WorkflowDiagram';
import {
  computeWorkflowDepths,
  computeWorkflowDagLayout,
  WORKFLOW_NODE_WIDTH,
} from './WorkflowDiagram.model';

let container: HTMLDivElement;
let root: Root | null = null;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) {
    await act(async () => { root!.unmount(); });
    root = null;
  }
  container.remove();
});

function render(element: React.ReactElement): void {
  act(() => { root!.render(element); });
}

describe('computeWorkflowDepths', () => {
  it('assigns the longest dependency-chain depth', () => {
    const nodes = [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }, { id: 'c', label: 'C' }];
    const edges = [{ from: 'a', to: 'b' }, { from: 'b', to: 'c' }];
    const depths = computeWorkflowDepths(nodes, edges);
    expect(depths.get('a')).toBe(0);
    expect(depths.get('b')).toBe(1);
    expect(depths.get('c')).toBe(2);
  });

  it('uses the longest path for fan-in nodes', () => {
    const nodes = [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }, { id: 'c', label: 'C' }, { id: 'd', label: 'D' }];
    const edges = [{ from: 'a', to: 'b' }, { from: 'a', to: 'c' }, { from: 'b', to: 'd' }, { from: 'c', to: 'd' }];
    const depths = computeWorkflowDepths(nodes, edges);
    expect(depths.get('d')).toBe(2);
  });

  it('is cycle-safe (does not recurse forever)', () => {
    const nodes = [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }];
    const edges = [{ from: 'a', to: 'b' }, { from: 'b', to: 'a' }];
    const depths = computeWorkflowDepths(nodes, edges);
    expect(typeof depths.get('a')).toBe('number');
    expect(typeof depths.get('b')).toBe('number');
  });
});

describe('computeWorkflowDagLayout', () => {
  it('places nodes in dependency-depth columns and emits bezier edges', () => {
    const nodes = [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }, { id: 'c', label: 'C' }];
    const edges = [{ from: 'a', to: 'b' }, { from: 'b', to: 'c' }];
    const layout = computeWorkflowDagLayout(nodes, edges);
    expect(layout.nodes).toHaveLength(3);
    expect(layout.edges).toHaveLength(2);

    const nodeA = layout.nodes.find((n) => n.node.id === 'a')!;
    const nodeB = layout.nodes.find((n) => n.node.id === 'b')!;
    const nodeC = layout.nodes.find((n) => n.node.id === 'c')!;
    // deeper dependency nodes sit in a later (higher x) column
    expect(nodeB.x).toBeGreaterThan(nodeA.x);
    expect(nodeC.x).toBeGreaterThan(nodeB.x);
    // edge path starts at the source node's right edge midpoint
    expect(layout.edges[0].path.startsWith(`M${nodeA.x + WORKFLOW_NODE_WIDTH} `)).toBe(true);
    expect(layout.width).toBeGreaterThan(0);
  });

  it('returns a zero-size layout when there are no nodes', () => {
    const layout = computeWorkflowDagLayout([], []);
    expect(layout.width).toBe(0);
    expect(layout.height).toBe(0);
    expect(layout.nodes).toHaveLength(0);
  });
});

describe('WorkflowDiagram component', () => {
  it('renders one node div per node and one svg path per real edge', () => {
    render(<WorkflowDiagram
      nodes={[{ id: 'a', label: 'A', agent: 'agentic' }, { id: 'b', label: 'B', agent: 'Plan' }]}
      edges={[{ from: 'a', to: 'b' }]}
      gateLabel="Gate"
      emptyLabel="No nodes"
    />);
    expect(container.querySelector('[data-testid="workflow-diagram"]')).not.toBeNull();
    expect(container.querySelectorAll('[data-node-id]')).toHaveLength(2);
    expect(container.querySelectorAll('path')).toHaveLength(1);
  });

  it('shows the empty label when there are no nodes', () => {
    render(<WorkflowDiagram nodes={[]} edges={[]} emptyLabel="No nodes" />);
    expect(container.querySelector('.workflow-diagram__empty')?.textContent).toBe('No nodes');
  });
});

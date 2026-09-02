/**
 * WorkflowDiagram — read-only SVG DAG for legion orchestration patterns.
 *
 * Scope guard (owner red-line): this is DISPLAY-ONLY — it renders the real
 * nodes/edges relationship graph. It does NOT support drag/rewire/replace, and
 * it introduces ZERO new npm dependencies (no @xyflow/react).
 *
 * The pure layout model lives in `WorkflowDiagram.model.ts` so this file only
 * exports React components.
 */
import React, { useMemo } from 'react';
import {
  computeWorkflowDagLayout,
  WORKFLOW_NODE_HEIGHT,
  WORKFLOW_NODE_WIDTH,
  type WorkflowDiagramEdge,
  type WorkflowDiagramNode,
} from './WorkflowDiagram.model';
import './WorkflowDiagram.scss';

interface WorkflowDiagramProps {
  nodes: Array<WorkflowDiagramNode>;
  edges: Array<WorkflowDiagramEdge>;
  /** Translated label for the gate marker (e.g. `t('legionPattern.gate')`). */
  gateLabel?: string;
  /** Translated empty-state message (e.g. `t('legionPattern.noNodes')`). */
  emptyLabel?: string;
}

const WorkflowDiagram: React.FC<WorkflowDiagramProps> = ({ nodes, edges, gateLabel, emptyLabel }) => {
  const layout = useMemo(() => computeWorkflowDagLayout(nodes, edges), [nodes, edges]);

  if (layout.nodes.length === 0) {
    return (
      <div
        className="workflow-diagram workflow-diagram--empty"
        data-testid="workflow-diagram"
        data-bf-component="workflow-diagram"
        data-bf-part="empty"
      >
        {emptyLabel ? <p className="workflow-diagram__empty">{emptyLabel}</p> : null}
      </div>
    );
  }

  return (
    <div
      className="workflow-diagram"
      data-testid="workflow-diagram"
      style={{ width: layout.width, height: layout.height }}
      data-bf-component="workflow-diagram"
      data-bf-part="root"
    >
      <svg
        className="workflow-diagram__edges"
        width={layout.width}
        height={layout.height}
        aria-hidden="true"
        data-bf-component="workflow-diagram"
        data-bf-part="edges"
      >
        {layout.edges.map((edge, index) => (
          <path
            key={`${edge.from}:${edge.to}:${index}`}
            className="workflow-diagram__edge"
            d={edge.path}
            data-from={edge.from}
            data-to={edge.to}
          />
        ))}
      </svg>
      {layout.nodes.map(({ node, x, y }) => (
        <div
          key={node.id}
          className="workflow-diagram__node"
          style={{ left: x, top: y, width: WORKFLOW_NODE_WIDTH, height: WORKFLOW_NODE_HEIGHT }}
          data-node-id={node.id}
          data-bf-component="workflow-diagram"
          data-bf-part="node"
        >
          <span className="workflow-diagram__node-label" title={node.label} data-bf-component="workflow-diagram" data-bf-part="nodeLabel">
            {node.label}
          </span>
          {node.agent ? (
            <span className="workflow-diagram__node-desc" title={node.agent}>
              {node.agent}
            </span>
          ) : null}
          {node.gate && gateLabel ? (
            <span className="workflow-diagram__node-gate">{gateLabel}</span>
          ) : null}
        </div>
      ))}
    </div>
  );
};

export default WorkflowDiagram;

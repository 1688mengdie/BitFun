import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const workflowDiagramAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'workflow-diagram',
  parts: [
    { id: 'root' },
    { id: 'edges' },
    { id: 'node' },
    { id: 'nodeLabel' },
    { id: 'empty' },
  ],
};

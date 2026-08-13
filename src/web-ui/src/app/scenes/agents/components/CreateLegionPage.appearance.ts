import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const createLegionPageAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'create-legion-page',
  parts: [
    { id: 'root' },
    { id: 'header' },
    { id: 'section' },
    { id: 'patternGrid' },
    { id: 'patternChip' },
    { id: 'patternChipIcon' },
    { id: 'summary' },
    { id: 'nodes' },
    { id: 'edges' },
    { id: 'actions' },
  ],
};

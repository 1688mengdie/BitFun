import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const nodePaletteAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'node-palette',
  parts: [
    { id: 'root' },
    { id: 'title' },
    { id: 'section' },
    { id: 'option' },
  ],
};

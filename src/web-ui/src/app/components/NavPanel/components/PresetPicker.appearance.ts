import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const presetPickerAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'preset-picker',
  parts: [
    { id: 'root' },
    { id: 'header' },
    { id: 'optionList' },
    { id: 'option' },
    { id: 'preview' },
  ],
};

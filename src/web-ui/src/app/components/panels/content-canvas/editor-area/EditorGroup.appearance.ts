import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasEditorGroupAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-editor-group',
  parts: [{ id: 'root' }, { id: 'content' }, { id: 'tabContent' }, { id: 'empty' }, { id: 'emptyContent' }],
  facets: [{
    id: 'group',
    attribute: 'data-bf-group',
    values: [
      'primary', 'secondary', 'tertiary',
      'slot4', 'slot5', 'slot6', 'slot7', 'slot8', 'slot9', 'slot10',
      'slot11', 'slot12', 'slot13', 'slot14', 'slot15', 'slot16',
    ],
  }],
  states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } }],
};

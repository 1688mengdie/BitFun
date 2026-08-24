import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasTabBarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-tab-bar',
  parts: [
    { id: 'root', visualRole: 'toolbar', continuityGroup: 'canvas-tabs' },
    { id: 'list', visualRole: 'continuous-surface', continuityGroup: 'canvas-tabs' },
    { id: 'tabWrapper', visualRole: 'continuous-surface', continuityGroup: 'canvas-tabs' },
    { id: 'dropIndicator', propertyProfile: 'overlay', visualRole: 'divider' },
    { id: 'actions', visualRole: 'toolbar' },
    { id: 'action', propertyProfile: 'control', visualRole: 'control' },
    { id: 'gridTemplate', propertyProfile: 'control', visualRole: 'control' },
    { id: 'gridTemplateMenu', propertyProfile: 'overlay', visualRole: 'popup' },
    { id: 'gridTemplateItem', propertyProfile: 'control', visualRole: 'control' },
    { id: 'gridTemplateExit', propertyProfile: 'control', visualRole: 'control' },
  ],
  facets: [{
    id: 'group',
    attribute: 'data-bf-group',
    // The `group` facet enumerates every editor-group slot this tab bar can render.
    // Beyond the legacy primary/secondary/tertiary, grid9 exposes slot4..slot16
    // (16 editor groups total) so the surface keeps complete group-facet coverage (P2-4).
    values: [
      'primary', 'secondary', 'tertiary',
      'slot4', 'slot5', 'slot6', 'slot7', 'slot8', 'slot9', 'slot10',
      'slot11', 'slot12', 'slot13', 'slot14', 'slot15', 'slot16',
    ],
  }],
  states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } }],
};

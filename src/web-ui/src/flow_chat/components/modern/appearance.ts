import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const virtualMessageListAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'virtual-message-list',
  parts: [
    { id: 'root' },
    { id: 'empty' },
    { id: 'emptyMessage' },
    { id: 'scroller' },
    { id: 'header' },
    { id: 'boundaryStatus' },
    { id: 'spacer' },
    { id: 'items' },
    { id: 'footer' },
    { id: 'handoffOverlay' },
    { id: 'handoffContent' },
  ],
  facets: [
    { id: 'renderMode', attribute: 'data-bf-render-mode', values: ['static', 'virtual'] },
    { id: 'position', attribute: 'data-bf-position', values: ['leading', 'trailing'] },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-bf-state~="empty"]' } },
    { id: 'preparing', selector: { kind: 'self', suffix: '[data-bf-state~="preparing"]' } },
    { id: 'unavailable', selector: { kind: 'self', suffix: '[data-bf-state~="unavailable"]' } },
  ],
};

export const modernFlowChatAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'modern-flow-chat',
  parts: [
    { id: 'root' },
    { id: 'messages' },
    { id: 'historyOverlay' },
    { id: 'historyOpenIntent' },
    { id: 'historyOpenIntentSpinner' },
  ],
};

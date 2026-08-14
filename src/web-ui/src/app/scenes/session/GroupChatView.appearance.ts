import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupChatViewAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-chat-view',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'header', visualRole: 'toolbar' },
    { id: 'memberBadge', propertyProfile: 'control', visualRole: 'content' },
    { id: 'membersToggle', propertyProfile: 'control', visualRole: 'control' },
    { id: 'members', propertyProfile: 'layout', visualRole: 'content', continuityGroup: 'session-workspace' },
    { id: 'memberRow', propertyProfile: 'control', visualRole: 'content' },
    { id: 'body', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'emptyState', visualRole: 'content' },
    { id: 'input', propertyProfile: 'layout', visualRole: 'toolbar' },
  ],
};

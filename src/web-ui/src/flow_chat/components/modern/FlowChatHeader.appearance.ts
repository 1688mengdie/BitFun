import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const flowChatHeaderAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'flow-chat-header',
  parts: [
    { id: 'root' },
    { id: 'leftActions' },
    { id: 'message' },
    { id: 'turnBadge' },
    { id: 'actions' },
    { id: 'backgroundActivity' },
    { id: 'activityPanel' },
    { id: 'activitySection' },
    { id: 'commandMenu' },
    { id: 'commandItem' },
    { id: 'search' },
    { id: 'searchControls' },
  ],
};

/**
 * ChatInput configuration constants
 */

export const CHAT_INPUT_CONFIG = {
  // Image input constraints.
  image: {
    maxCount: 5,
    acceptedTypes: ['image/png', 'image/jpeg', 'image/jpg', 'image/gif', 'image/webp'] as const,
  },
  
  // Mode sync delay in milliseconds.
  mode: {
    syncDelay: 200,
  },
} as const;

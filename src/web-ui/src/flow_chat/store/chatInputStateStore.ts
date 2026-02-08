/**
 * ChatInput state store for sharing expand/collapse state across components
 */

import { create } from 'zustand';

interface ChatInputStateStore {
  /** Whether ChatInput is active (transformed from collapsed capsule to normal input) */
  isActive: boolean;
  /** Whether ChatInput is expanded (full height mode) */
  isExpanded: boolean;
  
  setActive: (isActive: boolean) => void;
  setExpanded: (isExpanded: boolean) => void;
}

export const useChatInputState = create<ChatInputStateStore>((set) => ({
  isActive: true,
  isExpanded: false,
  
  setActive: (isActive) => set({ isActive }),
  setExpanded: (isExpanded) => set({ isExpanded }),
}));


/**
 * Template state reducer
 */

import type { PlaceholderFillState } from '@/shared/types/prompt-template';

export interface TemplateState {
  /** Template picker open state */
  isPickerOpen: boolean;
  /** Current placeholder fill state */
  fillState: PlaceholderFillState | null;
}

export type TemplateAction =
  | { type: 'OPEN_PICKER' }
  | { type: 'CLOSE_PICKER' }
  | { type: 'TOGGLE_PICKER' }
  | { type: 'START_FILL'; payload: PlaceholderFillState }
  | { type: 'EXIT_FILL' }
  | { type: 'UPDATE_CURRENT_INDEX'; payload: number }
  | { type: 'NEXT_PLACEHOLDER' }
  | { type: 'PREV_PLACEHOLDER' };

export const initialTemplateState: TemplateState = {
  isPickerOpen: false,
  fillState: null,
};

export function templateReducer(state: TemplateState, action: TemplateAction): TemplateState {
  switch (action.type) {
    case 'OPEN_PICKER':
      return { ...state, isPickerOpen: true };
      
    case 'CLOSE_PICKER':
      return { ...state, isPickerOpen: false };
      
    case 'TOGGLE_PICKER':
      return { ...state, isPickerOpen: !state.isPickerOpen };
      
    case 'START_FILL':
      return { ...state, fillState: action.payload };
      
    case 'EXIT_FILL':
      return { ...state, fillState: null };
      
    case 'UPDATE_CURRENT_INDEX':
      if (!state.fillState) return state;
      return {
        ...state,
        fillState: {
          ...state.fillState,
          currentIndex: action.payload,
        },
      };
      
    case 'NEXT_PLACEHOLDER':
      if (!state.fillState) return state;
      const nextIndex = state.fillState.currentIndex + 1;
      if (nextIndex >= state.fillState.placeholders.length) {
        // Reached the last placeholder; exit fill mode
        return { ...state, fillState: null };
      }
      return {
        ...state,
        fillState: {
          ...state.fillState,
          currentIndex: nextIndex,
        },
      };
      
    case 'PREV_PLACEHOLDER':
      if (!state.fillState) return state;
      const prevIndex = state.fillState.currentIndex - 1;
      if (prevIndex < 0) return state;
      return {
        ...state,
        fillState: {
          ...state.fillState,
          currentIndex: prevIndex,
        },
      };
      
    default:
      return state;
  }
}


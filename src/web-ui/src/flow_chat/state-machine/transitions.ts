/**
 * State transition table definition (three-state design)
 */

import { SessionExecutionState, SessionExecutionEvent, StateTransitionTable, ProcessingPhase } from './types';

/**
 * Minimal three-state transition table
 * 
 * Design philosophy:
 * - IDLE: idle, can start new task
 * - PROCESSING: running, can be cancelled or error
 * - ERROR: error state, can reset or retry
 * 
 * Cancellation logic: USER_CANCEL → immediately switch to IDLE (no backend wait)
 */
export const STATE_TRANSITIONS: StateTransitionTable = {
  [SessionExecutionState.IDLE]: {
    [SessionExecutionEvent.START]: SessionExecutionState.PROCESSING,
  },
  
  [SessionExecutionState.PROCESSING]: {
    [SessionExecutionEvent.USER_CANCEL]: SessionExecutionState.IDLE,
    
    [SessionExecutionEvent.ERROR_OCCURRED]: SessionExecutionState.ERROR,
    
    [SessionExecutionEvent.STREAM_COMPLETE]: SessionExecutionState.IDLE,
    
    [SessionExecutionEvent.MODEL_ROUND_START]: SessionExecutionState.PROCESSING,
    [SessionExecutionEvent.TEXT_CHUNK_RECEIVED]: SessionExecutionState.PROCESSING,
    [SessionExecutionEvent.TOOL_DETECTED]: SessionExecutionState.PROCESSING,
    [SessionExecutionEvent.TOOL_STARTED]: SessionExecutionState.PROCESSING,
    [SessionExecutionEvent.TOOL_COMPLETED]: SessionExecutionState.PROCESSING,
    [SessionExecutionEvent.TOOL_CONFIRMATION_NEEDED]: SessionExecutionState.PROCESSING,
    [SessionExecutionEvent.TOOL_CONFIRMED]: SessionExecutionState.PROCESSING,
    [SessionExecutionEvent.TOOL_REJECTED]: SessionExecutionState.IDLE,
  },
  
  [SessionExecutionState.ERROR]: {
    [SessionExecutionEvent.RESET]: SessionExecutionState.IDLE,
    [SessionExecutionEvent.START]: SessionExecutionState.PROCESSING,
  },
};

/**
 * Processing phase transitions (only valid in PROCESSING state)
 * Does not trigger main state change, only updates context.processingPhase
 */
export const PHASE_TRANSITIONS: Record<SessionExecutionEvent, ProcessingPhase | null> = {
  [SessionExecutionEvent.START]: ProcessingPhase.STARTING,
  [SessionExecutionEvent.MODEL_ROUND_START]: ProcessingPhase.THINKING,
  [SessionExecutionEvent.TEXT_CHUNK_RECEIVED]: ProcessingPhase.STREAMING,
  [SessionExecutionEvent.TOOL_DETECTED]: ProcessingPhase.TOOL_CALLING,
  [SessionExecutionEvent.TOOL_STARTED]: ProcessingPhase.TOOL_CALLING,
  [SessionExecutionEvent.TOOL_COMPLETED]: null,
  [SessionExecutionEvent.TOOL_CONFIRMATION_NEEDED]: ProcessingPhase.TOOL_CONFIRMING,
  [SessionExecutionEvent.TOOL_CONFIRMED]: ProcessingPhase.TOOL_CALLING,
  [SessionExecutionEvent.TOOL_REJECTED]: null,
  [SessionExecutionEvent.STREAM_COMPLETE]: null,
  [SessionExecutionEvent.USER_CANCEL]: null,
  [SessionExecutionEvent.ERROR_OCCURRED]: null,
  [SessionExecutionEvent.RESET]: null,
};

export function canTransition(
  from: SessionExecutionState,
  event: SessionExecutionEvent
): boolean {
  return STATE_TRANSITIONS[from]?.[event] !== undefined;
}

export function getNextState(
  from: SessionExecutionState,
  event: SessionExecutionEvent
): SessionExecutionState | null {
  return STATE_TRANSITIONS[from]?.[event] || null;
}

export function getPossibleEvents(
  state: SessionExecutionState
): SessionExecutionEvent[] {
  const transitions = STATE_TRANSITIONS[state];
  return transitions ? Object.keys(transitions) as SessionExecutionEvent[] : [];
}


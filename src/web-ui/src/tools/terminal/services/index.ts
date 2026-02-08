/**
 * Terminal service exports.
 */

export { TerminalService, getTerminalService } from './TerminalService';

export { 
  terminalActionManager,
  registerTerminalActions,
  unregisterTerminalActions,
  type TerminalActionHandler 
} from './TerminalActionManager';

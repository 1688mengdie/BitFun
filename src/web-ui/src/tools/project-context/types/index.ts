/**
 * Project Context panel type definitions.
 */

export * from './context';
export * from './knowledge';

export interface ProjectContextPanelProps {
  workspacePath: string;
  isActive?: boolean;
  onActivate?: () => void;
  className?: string;
}


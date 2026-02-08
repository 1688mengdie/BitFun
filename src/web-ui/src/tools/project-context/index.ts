/**
 * Project Context exports.
 * Manages project background info that affects agent output quality.
 *
 * Note: core services moved to the backend (crates/core/src/service/project_context/).
 */

export { ProjectContextPanel } from './components';

export { useProjectContextConfig, type ProjectContextConfig, type ProjectContextModule, type ModuleState } from './hooks';

export type { ProjectContextPanelProps } from './types';

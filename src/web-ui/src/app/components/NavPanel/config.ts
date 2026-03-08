/**
 * NAV_SECTIONS — pure data config for NavPanel navigation.
 *
 * behavior:'contextual' → stays in current scene, updates AuxPane / inline section
 * behavior:'scene'      → opens / activates a SceneBar tab
 *
 * Section groups:
 *   - workspace: project workspace essentials (sessions, files)
 *   - my-agent:  everything describing the super agent (profile, agents)
 */

import type { NavSection } from './types';

export const NAV_SECTIONS: NavSection[] = [
  {
    id: 'toolbox',
    label: 'Toolbox',
    collapsible: false,
    sceneId: 'toolbox',
    items: [],
  },
  {
    id: 'workspace',
    label: 'Workspace',
    collapsible: false,
    items: [],
  },
];

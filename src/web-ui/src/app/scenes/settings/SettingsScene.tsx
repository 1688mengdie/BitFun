/**
 * SettingsScene — content-only renderer for the Settings scene.
 *
 * The left-side navigation lives in SettingsNav (rendered by NavPanel via
 * nav-registry). This component only renders the active config content panel
 * driven by settingsStore.activeTab.
 */

import React from 'react';
import { useSettingsStore } from './settingsStore';
import './SettingsScene.scss';
import AIModelConfig from '../../../infrastructure/config/components/AIModelConfig';
import SessionConfig from '../../../infrastructure/config/components/SessionConfig';
import AIRulesMemoryConfig from '../../../infrastructure/config/components/AIRulesMemoryConfig';
import McpToolsConfig from '../../../infrastructure/config/components/McpToolsConfig';
import EditorConfig from '../../../infrastructure/config/components/EditorConfig';
import BasicsConfig from '../../../infrastructure/config/components/BasicsConfig';

const SettingsScene: React.FC = () => {
  const activeTab = useSettingsStore(s => s.activeTab);

  let Content: React.ComponentType | null = null;

  switch (activeTab) {
    case 'basics':           Content = BasicsConfig;         break;
    case 'models':           Content = AIModelConfig;        break;
    case 'session-config':   Content = SessionConfig;        break;
    case 'ai-context':       Content = AIRulesMemoryConfig; break;
    case 'mcp-tools':        Content = McpToolsConfig;      break;
    case 'editor':           Content = EditorConfig;         break;
  }

  return (
    <div className="bitfun-settings-scene">
      {Content && (
        <div key={activeTab} className="bitfun-settings-scene__content-wrapper">
          <Content />
        </div>
      )}
    </div>
  );
};

export default SettingsScene;

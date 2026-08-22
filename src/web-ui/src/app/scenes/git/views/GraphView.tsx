/**
 * GraphView — Wraps GitGraphView for the Git scene graph tab.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { GitGraphView } from '@/tools/git/components/GitGraphView';
import './GraphView.scss';

interface GraphViewProps {
  workspacePath?: string;
}

const GraphView: React.FC<GraphViewProps> = ({ workspacePath = '' }) => {
  const { t } = useTranslation('panels/git');
  if (!workspacePath) {
    return (
      <div data-bf-component="git-graph-view" data-bf-part="root" data-bf-state="empty" className="bitfun-git-scene-graph bitfun-git-scene-graph--empty">
        <p>{t('commitGraphEmpty')}</p>
      </div>
    );
  }

  return (
    <div data-bf-component="git-graph-view" data-bf-part="root" className="bitfun-git-scene-graph">
      <GitGraphView repositoryPath={workspacePath} />
    </div>
  );
};

export default GraphView;

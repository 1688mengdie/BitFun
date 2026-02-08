/**
 * Project Context control bar component.
 */

import React, { useState } from 'react';
import { 
  FileText, 
  ScrollText, 
  Layers2, 
  BookOpen,
  Star
} from 'lucide-react';
import { useProjectContextConfig, ProjectContextModule } from '../../hooks/useProjectContextConfig';
import { Switch } from '@/component-library';
import './ProjectContextControlBar.scss';

export interface ProjectContextControlBarProps {
  workspacePath: string;
  expanded: boolean;
  onToggle: () => void;
}

const MODULE_CONFIG: Record<ProjectContextModule, { icon: React.ReactNode; label: string }> = {
  docs: { icon: <FileText size={14} />, label: 'Guides' },
  rules: { icon: <ScrollText size={14} />, label: 'Coding standards' },
  architecture: { icon: <Layers2 size={14} />, label: 'Architecture view' },
  knowledge: { icon: <BookOpen size={14} />, label: 'Knowledge base' }
};

/**
 * Priority stars component.
 */
const PriorityStars: React.FC<{ 
  priority: number; 
  onChange: (priority: number) => void;
  disabled?: boolean;
}> = ({ priority, onChange, disabled }) => {
  const [hoveredStar, setHoveredStar] = useState<number | null>(null);
  
  return (
    <div className="bitfun-priority-stars">
      {[1, 2, 3, 4, 5].map((star) => (
        <button
          key={star}
          className={`bitfun-priority-stars__star ${
            star <= (hoveredStar ?? priority) ? 'bitfun-priority-stars__star--filled' : ''
          }`}
          onClick={() => !disabled && onChange(star)}
          onMouseEnter={() => !disabled && setHoveredStar(star)}
          onMouseLeave={() => setHoveredStar(null)}
          disabled={disabled}
          title={`Priority ${star}`}
        >
          <Star size={12} />
        </button>
      ))}
    </div>
  );
};

/**
 * Token budget progress bar.
 */
const TokenBudgetBar: React.FC<{
  used: number;
  budget: number;
}> = ({ used, budget }) => {
  const percentage = Math.min((used / budget) * 100, 100);
  const isOverBudget = used > budget;
  
  return (
    <div className="bitfun-token-budget">
      <div className="bitfun-token-budget__label">
        Token budget
      </div>
      <div className="bitfun-token-budget__bar">
        <div 
          className={`bitfun-token-budget__fill ${isOverBudget ? 'bitfun-token-budget__fill--over' : ''}`}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <div className={`bitfun-token-budget__text ${isOverBudget ? 'bitfun-token-budget__text--over' : ''}`}>
        {(used / 1000).toFixed(1)}k / {(budget / 1000).toFixed(0)}k
      </div>
    </div>
  );
};

/**
 * Project Context control bar.
 */
export const ProjectContextControlBar: React.FC<ProjectContextControlBarProps> = ({
  workspacePath,
  expanded
}) => {
  const { 
    config, 
    loading, 
    toggleModule, 
    updatePriority,
    getTokenUsage
  } = useProjectContextConfig(workspacePath);

  const tokenUsage = getTokenUsage();

  if (!expanded) {
    return null;
  }

  if (loading) {
    return (
      <div className="bitfun-project-context-control-bar bitfun-project-context-control-bar--loading">
        <div className="bitfun-project-context-control-bar__loading-text">
          Loading configuration...
        </div>
      </div>
    );
  }

  return (
    <div className="bitfun-project-context-control-bar">
      <TokenBudgetBar used={tokenUsage} budget={config.tokenBudget} />

      <div className="bitfun-project-context-control-bar__modules">
        {(Object.keys(MODULE_CONFIG) as ProjectContextModule[]).map((moduleKey) => {
          const moduleConfig = MODULE_CONFIG[moduleKey];
          const moduleState = config.modules[moduleKey];
          
          return (
            <div 
              key={moduleKey}
              className={`bitfun-project-context-control-bar__module ${
                !moduleState.enabled ? 'bitfun-project-context-control-bar__module--disabled' : ''
              }`}
            >
              <div className="bitfun-project-context-control-bar__module-toggle">
                <span className="bitfun-project-context-control-bar__module-icon">
                  {moduleConfig.icon}
                </span>
                <span className="bitfun-project-context-control-bar__module-label">
                  {moduleConfig.label}
                </span>
                {moduleState.enabledCount !== undefined && (
                  <span className="bitfun-project-context-control-bar__module-count">
                    ({moduleState.enabledCount}/{moduleState.totalCount})
                  </span>
                )}
                <Switch
                  size="small"
                  checked={moduleState.enabled}
                  onChange={() => toggleModule(moduleKey)}
                />
              </div>

              <div className="bitfun-project-context-control-bar__module-priority">
                <PriorityStars
                  priority={moduleState.priority}
                  onChange={(p) => updatePriority(moduleKey, p)}
                  disabled={!moduleState.enabled}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default ProjectContextControlBar;

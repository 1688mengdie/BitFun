import React from 'react';
import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  useAgentsStore,
  MOCK_AGENTS,
  CAPABILITY_CATEGORIES,
  CAPABILITY_COLORS,
  computeAgentTeamCapabilities,
  type AgentWithCapabilities,
  type CapabilityCategory,
} from '../agentsStore';
import './CapabilityBar.scss';

const CapabilityBar: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { agentTeams, activeAgentTeamId } = useAgentsStore();
  const team = agentTeams.find((t) => t.id === activeAgentTeamId);
  if (!team) return null;

  const coverage = computeAgentTeamCapabilities(team, MOCK_AGENTS as AgentWithCapabilities[]);
  const weak     = CAPABILITY_CATEGORIES.filter((c) => coverage[c] === 0);

  return (
    <div className="cap-bar">
      <span className="cap-bar__label">{t('capability.coverage', '能力覆盖')}</span>

      <div className="cap-bar__items">
        {CAPABILITY_CATEGORIES.map((cat) => {
          const level = coverage[cat];
          const color = CAPABILITY_COLORS[cat as CapabilityCategory];
          const pct   = Math.round((level / 5) * 100);
          return (
            <div
              key={cat}
              className="cap-bar__item"
              title={`${cat}：${level > 0 ? `Lv${level}` : t('capability.none', '无覆盖')}`}
            >
              <span className="cap-bar__cat">{cat}</span>
              <div className="cap-bar__track">
                <div
                  className="cap-bar__fill"
                  style={{ width: `${pct}%`, background: level > 0 ? color : undefined }}
                />
              </div>
              <span
                className="cap-bar__lv"
                style={level > 0 ? { color } : undefined}
              >
                {level > 0 ? level : '—'}
              </span>
            </div>
          );
        })}
      </div>

      {weak.length > 0 && (
        <div className="cap-bar__warn">
          <AlertTriangle size={10} />
          {t('capability.warning', { cats: weak.join('、') })}
        </div>
      )}
    </div>
  );
};

export default CapabilityBar;

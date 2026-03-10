import React from 'react';
import { Bot, Pencil, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/component-library';
import type { AgentTeam, AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP, CAPABILITY_ACCENT, AGENT_TEAM_ICON_MAP, getAgentTeamAccent } from '../agentsIcons';
import './AgentTeamCard.scss';

interface AgentTeamCardProps {
  team: AgentTeam;
  allAgents: AgentWithCapabilities[];
  index?: number;
  isExample?: boolean;
  onEdit: (teamId: string) => void;
  onOpenDetails: (team: AgentTeam) => void;
  topCapabilities: string[];
}

const AgentTeamCard: React.FC<AgentTeamCardProps> = ({
  team,
  allAgents,
  index = 0,
  isExample = false,
  onEdit,
  onOpenDetails,
  topCapabilities,
}) => {
  const { t } = useTranslation('scenes/agents');
  const Icon = AGENT_TEAM_ICON_MAP[team.icon as keyof typeof AGENT_TEAM_ICON_MAP] ?? Users;
  const accent = getAgentTeamAccent(team.id);
  const memberAgents = team.members
    .map((member) => allAgents.find((agent) => agent.id === member.agentId))
    .filter(Boolean) as AgentWithCapabilities[];

  const strategyLabel =
    team.strategy === 'collaborative'
      ? t('composer.strategy.collaborative')
      : team.strategy === 'sequential'
        ? t('composer.strategy.sequential')
        : t('composer.strategy.free');

  const openDetails = () => onOpenDetails(team);

  return (
    <div
      className="agent-team-card"
      style={{
        '--card-index': index,
        '--agent-team-card-accent': accent,
      } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && openDetails()}
      aria-label={team.name}
    >
      <div className="agent-team-card__icon-area">
        <div className="agent-team-card__icon">
          <Icon size={24} strokeWidth={1.7} />
        </div>
      </div>

      <div className="agent-team-card__body">
        <div className="agent-team-card__header">
          <div className="agent-team-card__header-main">
            <div className="agent-team-card__title-row">
              <span className="agent-team-card__name">{team.name}</span>
            </div>
            <p className="agent-team-card__desc">{team.description?.trim() || '—'}</p>
          </div>

          <div className="agent-team-card__actions" onClick={(e) => e.stopPropagation()}>
            <button
              type="button"
              className="agent-team-card__icon-btn"
              onClick={() => onEdit(team.id)}
              aria-label={t('home.edit')}
              title={t('home.edit')}
            >
              <Pencil size={14} />
            </button>
          </div>
        </div>

        <div className="agent-team-card__meta">
          <div className="agent-team-card__avatars">
            {memberAgents.slice(0, 4).map((agent) => {
              const AgentIcon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
              return (
                <span key={agent.id} className="agent-team-card__avatar" title={agent.name}>
                  <AgentIcon size={10} />
                </span>
              );
            })}
            {team.members.length > 4 ? (
              <span className="agent-team-card__avatar agent-team-card__avatar--more">
                +{team.members.length - 4}
              </span>
            ) : null}
          </div>
          <span className="agent-team-card__meta-item">
            {t('home.members', { count: team.members.length })}
          </span>
          {topCapabilities.length > 0 ? (
            <div className="agent-team-card__cap-chips">
              {topCapabilities.map((cap) => (
                <span
                  key={cap}
                  className="agent-team-card__cap-chip"
                  style={{
                    color: CAPABILITY_ACCENT[cap],
                    borderColor: `${CAPABILITY_ACCENT[cap]}44`,
                  }}
                >
                  {cap}
                </span>
              ))}
            </div>
          ) : null}
          <div className="agent-team-card__state-badges">
            {isExample ? <Badge variant="neutral">{t('teamCard.badges.example', '示例')}</Badge> : null}
            <Badge variant="neutral">{strategyLabel}</Badge>
            {team.shareContext ? (
              <Badge variant="success">{t('teamCard.badges.sharedContext', '共享上下文')}</Badge>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
};

export default AgentTeamCard;

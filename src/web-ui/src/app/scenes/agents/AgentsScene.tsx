import React, { useCallback, useMemo } from 'react';
import {
  ArrowLeft,
  Bot,
  Cpu,
  Plus,
  Puzzle,
  RefreshCw,
  Search as SearchIcon,
  Users,
  Wrench,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, IconButton, Search } from '@/component-library';
import {
  GalleryDetailModal,
  GalleryEmpty,
  GalleryGrid,
  GalleryLayout,
  GalleryPageHeader,
  GallerySkeleton,
  GalleryZone,
} from '@/app/components';
import AgentCard from './components/AgentCard';
import CoreAgentCard, { type CoreAgentMeta } from './components/CoreAgentCard';
import AgentTeamCard from './components/AgentTeamCard';
import AgentTeamTabBar from './components/AgentTeamTabBar';
import AgentGallery from './components/AgentGallery';
import AgentTeamComposer from './components/AgentTeamComposer';
import CapabilityBar from './components/CapabilityBar';
import CreateAgentPage from './components/CreateAgentPage';
import {
  CAPABILITY_CATEGORIES,
  MOCK_AGENT_TEAMS,
  computeAgentTeamCapabilities,
  type AgentWithCapabilities,
  useAgentsStore,
} from './agentsStore';
import { useAgentsList } from './hooks/useAgentsList';
import { AGENT_ICON_MAP, CAPABILITY_ACCENT, AGENT_TEAM_ICON_MAP, getAgentTeamAccent } from './agentsIcons';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { getAgentBadge } from './utils';
import './AgentsView.scss';
import './AgentsScene.scss';

const EXAMPLE_TEAM_IDS = new Set(MOCK_AGENT_TEAMS.map((team) => team.id));

const CORE_AGENT_IDS = new Set(['Claw', 'agentic', 'Cowork']);

const CORE_AGENT_META: Record<string, CoreAgentMeta> = {
  Claw:    { role: '个人助理',       accentColor: '#f59e0b', accentBg: 'rgba(245,158,11,0.10)' },
  agentic: { role: '编码专业智能体', accentColor: '#6366f1', accentBg: 'rgba(99,102,241,0.10)' },
  Cowork:  { role: '办公智能体',     accentColor: '#14b8a6', accentBg: 'rgba(20,184,166,0.10)' },
};

const AgentTeamEditorView: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { openHome } = useAgentsStore();

  return (
    <div className="tv tv--editor">
      <div className="tv__editor-bar">
        <button className="tv__back-btn" onClick={openHome}>
          <ArrowLeft size={14} />
          <span>{t('home.backToOverview')}</span>
        </button>
      </div>

      <AgentTeamTabBar />

      <div className="tv__body">
        <aside className="tv__gallery">
          <div className="tv__panel-label">{t('gallery.title')}</div>
          <AgentGallery />
        </aside>

        <main className="tv__composer">
          <AgentTeamComposer />
        </main>
      </div>

      <CapabilityBar />
    </div>
  );
};

const AgentsHomeView: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const {
    agentTeams,
    agentSoloEnabled,
    searchQuery,
    agentFilterLevel,
    agentFilterType,
    setSearchQuery,
    setAgentFilterLevel,
    setAgentFilterType,
    setAgentSoloEnabled,
    openAgentTeamEditor,
    openCreateAgent,
    addAgentTeam,
  } = useAgentsStore();
  const [selectedAgentId, setSelectedAgentId] = React.useState<string | null>(null);
  const [selectedTeamId, setSelectedTeamId] = React.useState<string | null>(null);
  const [toolsEditing, setToolsEditing] = React.useState(false);
  const [skillsEditing, setSkillsEditing] = React.useState(false);

  const {
    allAgents,
    filteredAgents,
    loading,
    availableTools,
    availableSkills,
    counts,
    loadAgents,
    getModeConfig,
    handleToggleTool,
    handleResetTools,
    handleToggleSkill,
  } = useAgentsList({
    searchQuery,
    filterLevel: agentFilterLevel,
    filterType: agentFilterType,
    t,
  });

  const filteredTeams = useMemo(() => agentTeams.filter((team) => {
    if (!searchQuery) return true;
    const query = searchQuery.toLowerCase();
    return team.name.toLowerCase().includes(query) || team.description.toLowerCase().includes(query);
  }), [agentTeams, searchQuery]);

  const coreAgents = useMemo(() => allAgents.filter((agent) => CORE_AGENT_IDS.has(agent.id)), [allAgents]);

  const handleCreateTeam = useCallback(() => {
    const id = `agent-team-${Date.now()}`;
    addAgentTeam({
      id,
      name: t('teamsZone.newTeamName', '新团队'),
      icon: 'users',
      description: '',
      strategy: 'collaborative',
      shareContext: true,
    });
    openAgentTeamEditor(id);
  }, [addAgentTeam, openAgentTeamEditor, t]);

  const scrollToZone = useCallback((targetId: string) => {
    document.getElementById(targetId)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }, []);

  const levelFilters = [
    { key: 'builtin', label: t('filters.builtin', '内置'), count: counts.builtin },
    { key: 'user', label: t('filters.user', '用户'), count: counts.user },
    { key: 'project', label: t('filters.project', '项目'), count: counts.project },
  ] as const;

  const typeFilters = [
    { key: 'mode', label: t('filters.mode', 'Agent'), count: counts.mode },
    { key: 'subagent', label: t('filters.subagent', 'Sub-Agent'), count: counts.subagent },
  ] as const;

  const renderSkeletons = (prefix: string) => (
    <GallerySkeleton count={6} cardHeight={138} className={`${prefix}-skeleton`} />
  );

  const selectedAgent = useMemo(
    () => allAgents.find((agent) => agent.id === selectedAgentId) ?? null,
    [allAgents, selectedAgentId],
  );
  const selectedAgentModeConfig = useMemo(
    () => (selectedAgent?.agentKind === 'mode' ? getModeConfig(selectedAgent.id) : null),
    [getModeConfig, selectedAgent],
  );
  const selectedAgentTools = selectedAgent?.agentKind === 'mode'
    ? (selectedAgentModeConfig?.available_tools ?? selectedAgent.defaultTools ?? [])
    : (selectedAgent?.defaultTools ?? []);
  const selectedAgentSkills = selectedAgentModeConfig?.available_skills ?? [];
  const selectedAgentSkillItems = availableSkills.filter((skill) => selectedAgentSkills.includes(skill.name));
  const selectedTeam = useMemo(
    () => agentTeams.find((team) => team.id === selectedTeamId) ?? null,
    [agentTeams, selectedTeamId],
  );
  const selectedAgentTeamMembers = useMemo(
    () => selectedTeam
      ? selectedTeam.members
        .map((member) => allAgents.find((agent) => agent.id === member.agentId))
        .filter((agent): agent is AgentWithCapabilities => Boolean(agent))
      : [],
    [allAgents, selectedTeam],
  );
  const selectedTeamTopCaps = useMemo(() => {
    if (!selectedTeam) return [];
    const caps = computeAgentTeamCapabilities(selectedTeam, allAgents);
    return CAPABILITY_CATEGORIES
      .filter((category) => caps[category] > 0)
      .sort((a, b) => caps[b] - caps[a])
      .slice(0, 3);
  }, [allAgents, selectedTeam]);

  const openAgentDetails = useCallback((agent: AgentWithCapabilities) => {
    setSelectedTeamId(null);
    setSelectedAgentId(agent.id);
    setToolsEditing(false);
    setSkillsEditing(false);
  }, []);

  const closeAgentDetails = useCallback(() => {
    setSelectedAgentId(null);
    setToolsEditing(false);
    setSkillsEditing(false);
  }, []);

  const openTeamDetails = useCallback((teamId: string) => {
    setSelectedAgentId(null);
    setToolsEditing(false);
    setSkillsEditing(false);
    setSelectedTeamId(teamId);
  }, []);

  return (
    <GalleryLayout className="bitfun-agents-scene">
      <GalleryPageHeader
        title={t('page.title')}
        subtitle={t('page.subtitle')}
        extraContent={(
          <div className="gallery-anchor-bar">
            <button
              type="button"
              className="gallery-anchor-btn"
              onClick={() => scrollToZone('core-agents-zone')}
            >
              {t('nav.coreAgents')}
            </button>
            <button
              type="button"
              className="gallery-anchor-btn"
              onClick={() => scrollToZone('agents-zone')}
            >
              {t('nav.agents')}
            </button>
            <button
              type="button"
              className="gallery-anchor-btn"
              onClick={() => scrollToZone('agent-teams-zone')}
            >
              {t('nav.teams')}
            </button>
          </div>
        )}
        actions={(
          <>
            <Search
              value={searchQuery}
              onChange={setSearchQuery}
              placeholder={t('page.searchPlaceholder')}
              size="small"
              clearable
              prefixIcon={<></>}
              suffixContent={(
                <button
                  type="button"
                  className="gallery-search-btn"
                  aria-label={t('page.searchPlaceholder')}
                >
                  <SearchIcon size={14} />
                </button>
              )}
            />
            <button
              type="button"
              className="gallery-action-btn"
              onClick={() => void loadAgents()}
              disabled={loading}
              aria-label={t('page.refresh')}
              title={t('page.refresh')}
            >
              <RefreshCw
                size={15}
                className={loading ? 'gallery-spinning' : undefined}
              />
            </button>
          </>
        )}
      />

      <div className="gallery-zones">
        <GalleryZone
          id="core-agents-zone"
          title={t('coreAgentsZone.title')}
          subtitle={t('coreAgentsZone.subtitle')}
          tools={(
            <span className="gallery-zone-count">{coreAgents.length}</span>
          )}
        >
          {loading ? (
            <GallerySkeleton count={3} cardHeight={160} className="core-agent-skeleton" />
          ) : coreAgents.length === 0 ? (
            <GalleryEmpty
              icon={<Cpu size={32} strokeWidth={1.5} />}
              message={t('coreAgentsZone.empty')}
            />
          ) : (
            <div className="core-agents-grid">
              {coreAgents.map((agent, index) => (
                <CoreAgentCard
                  key={agent.id}
                  agent={agent}
                  index={index}
                  meta={CORE_AGENT_META[agent.id] ?? { role: agent.name, accentColor: '#6366f1', accentBg: 'rgba(99,102,241,0.10)' }}
                  skillCount={agent.agentKind === 'mode' ? (getModeConfig(agent.id)?.available_skills?.length ?? 0) : 0}
                  onOpenDetails={openAgentDetails}
                />
              ))}
            </div>
          )}
        </GalleryZone>

        <GalleryZone
          id="agents-zone"
          title={t('agentsZone.title')}
          subtitle={t('agentsZone.subtitle')}
          tools={(
            <>
              <div className="bitfun-agents-scene__agent-filters">
                <div className="bitfun-agents-scene__agent-filter-group">
                  <span className="bitfun-agents-scene__agent-filter-label">
                    {t('filters.source', '来源')}
                  </span>
                  {levelFilters.map(({ key, label, count }) => (
                    <button
                      key={key}
                      type="button"
                      className={[
                        'gallery-cat-chip',
                        agentFilterLevel === key && 'gallery-cat-chip--active',
                      ].filter(Boolean).join(' ')}
                      onClick={() => setAgentFilterLevel(agentFilterLevel === key ? 'all' : key)}
                    >
                      <span>{label}</span>
                      <span className="gallery-filter-count">{count}</span>
                    </button>
                  ))}
                </div>
                <div className="bitfun-agents-scene__agent-filter-group">
                  <span className="bitfun-agents-scene__agent-filter-label">
                    {t('filters.kind', '类型')}
                  </span>
                  {typeFilters.map(({ key, label, count }) => (
                    <button
                      key={key}
                      type="button"
                      className={[
                        'gallery-cat-chip',
                        agentFilterType === key && 'gallery-cat-chip--active',
                      ].filter(Boolean).join(' ')}
                      onClick={() => setAgentFilterType(agentFilterType === key ? 'all' : key)}
                    >
                      <span>{label}</span>
                      <span className="gallery-filter-count">{count}</span>
                    </button>
                  ))}
                </div>
              </div>
              <button
                type="button"
                className="gallery-action-btn gallery-action-btn--primary"
                onClick={openCreateAgent}
              >
                <Plus size={15} />
                <span>{t('page.newAgent')}</span>
              </button>
              <span className="gallery-zone-count">{filteredAgents.length}</span>
            </>
          )}
        >
          {loading ? renderSkeletons('agent') : null}

          {!loading && filteredAgents.length === 0 ? (
            <GalleryEmpty
              icon={<Bot size={32} strokeWidth={1.5} />}
              message={allAgents.length === 0 ? t('agentsZone.empty.noAgents') : t('agentsZone.empty.noMatch')}
            />
          ) : null}

          {!loading && filteredAgents.length > 0 ? (
            <GalleryGrid>
              {filteredAgents.map((agent, index) => (
                <AgentCard
                  key={agent.id}
                  agent={agent}
                  index={index}
                  soloEnabled={agentSoloEnabled[agent.id] ?? agent.enabled}
                  skillCount={agent.agentKind === 'mode' ? (getModeConfig(agent.id)?.available_skills?.length ?? 0) : 0}
                  onToggleSolo={setAgentSoloEnabled}
                  onOpenDetails={openAgentDetails}
                />
              ))}
            </GalleryGrid>
          ) : null}
        </GalleryZone>

        <GalleryZone
          id="agent-teams-zone"
          title={t('teamsZone.title')}
          subtitle={t('teamsZone.subtitle')}
          tools={(
            <>
              <button
                type="button"
                className="gallery-action-btn gallery-action-btn--primary"
                onClick={handleCreateTeam}
              >
                <Users size={15} />
                <span>{t('teamsZone.create')}</span>
              </button>
              <span className="gallery-zone-count">{filteredTeams.length}</span>
            </>
          )}
        >
          {filteredTeams.length === 0 ? (
            <GalleryEmpty
              icon={<Users size={32} strokeWidth={1.5} />}
              message={agentTeams.length === 0 ? t('teamsZone.empty.noTeams') : t('teamsZone.empty.noMatch')}
            />
          ) : (
            <GalleryGrid>
              {filteredTeams.map((team, index) => {
                const caps = computeAgentTeamCapabilities(team, allAgents);
                const topCaps = CAPABILITY_CATEGORIES
                  .filter((category) => caps[category] > 0)
                  .sort((a, b) => caps[b] - caps[a])
                  .slice(0, 3);

                return (
                  <AgentTeamCard
                    key={team.id}
                    team={team}
                    allAgents={allAgents}
                    index={index}
                    isExample={EXAMPLE_TEAM_IDS.has(team.id)}
                    onEdit={openAgentTeamEditor}
                    onOpenDetails={(currentTeam) => openTeamDetails(currentTeam.id)}
                    topCapabilities={topCaps}
                  />
                );
              })}
            </GalleryGrid>
          )}
        </GalleryZone>
      </div>

      <GalleryDetailModal
        isOpen={Boolean(selectedAgent)}
        onClose={closeAgentDetails}
        icon={selectedAgent ? React.createElement(
          AGENT_ICON_MAP[(selectedAgent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot,
          { size: 24, strokeWidth: 1.7 },
        ) : <Bot size={24} />}
        iconGradient={selectedAgent ? getCardGradient(selectedAgent.id || selectedAgent.name) : undefined}
        title={selectedAgent?.name ?? ''}
        badges={selectedAgent ? (
          <>
            <Badge variant={getAgentBadge(t, selectedAgent.agentKind, selectedAgent.subagentSource).variant}>
              {selectedAgent.agentKind === 'mode' ? <Cpu size={10} /> : <Bot size={10} />}
              {getAgentBadge(t, selectedAgent.agentKind, selectedAgent.subagentSource).label}
            </Badge>
            {!selectedAgent.enabled ? <Badge variant="neutral">{t('agentCard.badges.disabled', '已禁用')}</Badge> : null}
            {selectedAgent.model ? <Badge variant="neutral">{selectedAgent.model}</Badge> : null}
          </>
        ) : null}
        description={selectedAgent?.description}
        meta={selectedAgent ? (
          <>
            <span>{t('agentCard.meta.tools', '{{count}} 个工具', { count: selectedAgent.toolCount ?? selectedAgentTools.length })}</span>
            {selectedAgent.agentKind === 'mode' ? (
              <span>{t('agentCard.meta.skills', '{{count}} 个 Skills', { count: selectedAgentSkills.length })}</span>
            ) : null}
          </>
        ) : null}
      >
        {selectedAgent ? (
          <>
            <div className="agent-card__cap-grid">
              {selectedAgent.capabilities.map((cap) => (
                <div key={cap.category} className="agent-card__cap-row">
                  <span
                    className="agent-card__cap-label"
                    style={{ color: CAPABILITY_ACCENT[cap.category] }}
                  >
                    {cap.category}
                  </span>
                  <div className="agent-card__cap-bar">
                    {Array.from({ length: 5 }).map((_, i) => (
                      <span
                        key={i}
                        className="agent-card__cap-pip"
                        style={i < cap.level ? { backgroundColor: CAPABILITY_ACCENT[cap.category] } : undefined}
                      />
                    ))}
                  </div>
                  <span className="agent-card__cap-level">{cap.level}/5</span>
                </div>
              ))}
            </div>

            {selectedAgentTools.length > 0 ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <Wrench size={12} />
                    <span>{t('agentsOverview.tools', '工具')}</span>
                    <span className="agent-card__section-count">
                      {selectedAgent.agentKind === 'mode'
                        ? `${selectedAgentTools.length}/${availableTools.length}`
                        : `${selectedAgentTools.length}`}
                    </span>
                  </div>
                  {selectedAgent.agentKind === 'mode' ? (
                    <div className="agent-card__section-actions">
                      <Button variant="secondary" size="small" onClick={() => setToolsEditing((prev) => !prev)}>
                        {toolsEditing ? t('agentsOverview.toolsCancel', '完成管理') : t('agentsOverview.toolsEdit', '管理工具')}
                      </Button>
                      {toolsEditing ? (
                        <IconButton
                          size="small"
                          variant="ghost"
                          tooltip={t('agentsOverview.toolsReset', '重置默认')}
                          onClick={() => void handleResetTools(selectedAgent.id)}
                        >
                          <RefreshCw size={12} />
                        </IconButton>
                      ) : null}
                    </div>
                  ) : null}
                </div>

                {selectedAgent.agentKind === 'mode' && toolsEditing ? (
                  <div className="agent-card__token-grid">
                    {[...availableTools]
                      .sort((a, b) => {
                        const aOn = selectedAgentTools.includes(a.name);
                        const bOn = selectedAgentTools.includes(b.name);
                        if (aOn && !bOn) return -1;
                        if (!aOn && bOn) return 1;
                        return 0;
                      })
                      .map((tool) => {
                        const isOn = selectedAgentTools.includes(tool.name);
                        return (
                          <button
                            key={tool.name}
                            type="button"
                            className={`agent-card__token${isOn ? ' is-on' : ''}`}
                            title={tool.description || tool.name}
                            onClick={() => void handleToggleTool(selectedAgent.id, tool.name)}
                          >
                            <span className="agent-card__token-name">{tool.name}</span>
                          </button>
                        );
                      })}
                  </div>
                ) : (
                  <div className="agent-card__chip-grid">
                    {selectedAgentTools.map((tool) => (
                      <span key={tool} className="agent-card__chip" title={tool}>
                        {tool.replace(/_/g, ' ')}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            ) : null}

            {selectedAgent.agentKind === 'mode' && availableSkills.length > 0 ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <Puzzle size={12} />
                    <span>{t('agentsOverview.skills', 'Skills')}</span>
                    <span className="agent-card__section-count">
                      {`${selectedAgentSkills.length}/${availableSkills.length}`}
                    </span>
                  </div>
                  <div className="agent-card__section-actions">
                    <Button variant="secondary" size="small" onClick={() => setSkillsEditing((prev) => !prev)}>
                      {skillsEditing ? t('agentsOverview.skillsCancel', '完成管理') : t('agentsOverview.skillsEdit', '管理 Skills')}
                    </Button>
                  </div>
                </div>

                {skillsEditing ? (
                  <div className="agent-card__token-grid">
                    {[...availableSkills]
                      .sort((a, b) => {
                        const aOn = selectedAgentSkills.includes(a.name);
                        const bOn = selectedAgentSkills.includes(b.name);
                        if (aOn && !bOn) return -1;
                        if (!aOn && bOn) return 1;
                        return 0;
                      })
                      .map((skill) => {
                        const isOn = selectedAgentSkills.includes(skill.name);
                        return (
                          <button
                            key={skill.name}
                            type="button"
                            className={`agent-card__token${isOn ? ' is-on' : ''}`}
                            title={skill.description || skill.name}
                            onClick={() => void handleToggleSkill(selectedAgent.id, skill.name)}
                          >
                            <span className="agent-card__token-name">{skill.name}</span>
                          </button>
                        );
                      })}
                  </div>
                ) : (
                  <div className="agent-card__chip-grid">
                    {selectedAgentSkillItems.length === 0 ? (
                      <span className="agent-card__empty-inline">
                        {t('agentsOverview.noSkills', '未启用任何 Skill')}
                      </span>
                    ) : (
                      selectedAgentSkillItems.map((skill) => (
                        <span key={skill.name} className="agent-card__chip" title={skill.description || skill.name}>
                          {skill.name}
                        </span>
                      ))
                    )}
                  </div>
                )}
              </div>
            ) : null}
          </>
        ) : null}
      </GalleryDetailModal>

      <GalleryDetailModal
        isOpen={Boolean(selectedTeam)}
        onClose={() => setSelectedTeamId(null)}
        icon={selectedTeam ? React.createElement(
          AGENT_TEAM_ICON_MAP[(selectedTeam.icon ?? 'users') as keyof typeof AGENT_TEAM_ICON_MAP] ?? Users,
          { size: 24, strokeWidth: 1.7 },
        ) : <Users size={24} />}
        iconGradient={selectedTeam ? `linear-gradient(135deg, ${getAgentTeamAccent(selectedTeam.id)}33 0%, ${getAgentTeamAccent(selectedTeam.id)}14 100%)` : undefined}
        title={selectedTeam?.name ?? ''}
        badges={selectedTeam ? (
          <>
            {EXAMPLE_TEAM_IDS.has(selectedTeam.id) ? <Badge variant="neutral">{t('teamCard.badges.example', '示例')}</Badge> : null}
            <Badge variant="neutral">
              {selectedTeam.strategy === 'collaborative'
                ? t('composer.strategy.collaborative')
                : selectedTeam.strategy === 'sequential'
                  ? t('composer.strategy.sequential')
                  : t('composer.strategy.free')}
            </Badge>
            {selectedTeam.shareContext ? (
              <Badge variant="success">{t('teamCard.badges.sharedContext', '共享上下文')}</Badge>
            ) : null}
          </>
        ) : null}
        description={selectedTeam?.description}
        meta={selectedTeam ? <span>{t('home.members', { count: selectedTeam.members.length })}</span> : null}
        actions={selectedTeam ? (
          <Button
            variant="primary"
            size="small"
            onClick={() => {
              setSelectedTeamId(null);
              openAgentTeamEditor(selectedTeam.id);
            }}
          >
            {t('home.edit')}
          </Button>
        ) : null}
      >
        {selectedAgentTeamMembers.length > 0 ? (
          <div className="agent-team-card__section">
            <div className="agent-team-card__section-title">{t('teamCard.sections.members', '成员')}</div>
            <div className="agent-team-card__member-list">
              {selectedAgentTeamMembers.map((agent) => {
                const member = selectedTeam?.members.find((item) => item.agentId === agent.id);
                const roleLabel =
                  member?.role === 'leader'
                    ? t('composer.role.leader')
                    : member?.role === 'reviewer'
                      ? t('composer.role.reviewer')
                      : t('composer.role.member');
                const AgentIcon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;

                return (
                  <span key={agent.id} className="agent-team-card__member-chip">
                    <AgentIcon size={11} />
                    <span className="agent-team-card__member-name">{agent.name}</span>
                    <span className="agent-team-card__member-role">{roleLabel}</span>
                  </span>
                );
              })}
            </div>
          </div>
        ) : null}

        {selectedTeamTopCaps.length > 0 ? (
          <div className="agent-team-card__section">
            <div className="agent-team-card__section-title">{t('teamCard.sections.capabilities', '能力')}</div>
            <div className="agent-team-card__cap-chips">
              {selectedTeamTopCaps.map((cap) => (
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
          </div>
        ) : null}
      </GalleryDetailModal>
    </GalleryLayout>
  );
};

const AgentsScene: React.FC = () => {
  const { page } = useAgentsStore();

  if (page === 'editor') {
    return (
      <div className="bitfun-agents-scene">
        <AgentTeamEditorView />
      </div>
    );
  }

  if (page === 'createAgent') {
    return (
      <div className="bitfun-agents-scene">
        <CreateAgentPage />
      </div>
    );
  }

  return <AgentsHomeView />;
};

export default AgentsScene;

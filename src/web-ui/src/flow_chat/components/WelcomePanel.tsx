/**
 * Welcome panel shown in the empty chat state.
 * Layout mirrors WelcomeScene: centered container, left-aligned content.
 */

import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, ChevronDown, Check, GitBranch } from 'lucide-react';
import { gitAPI } from '../../infrastructure/api';
import type { GitWorkState } from '../../infrastructure/api/service-api/StartchatAgentAPI';
import { useApp } from '../../app/hooks/useApp';
import { createLogger } from '@/shared/utils/logger';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import type { WorkspaceInfo } from '@/shared/types';
import CoworkExampleCards from './CoworkExampleCards';
import './WelcomePanel.css';

const log = createLogger('WelcomePanel');

interface WelcomePanelProps {
  onQuickAction?: (command: string) => void;
  className?: string;
  sessionMode?: string;
}

export const WelcomePanel: React.FC<WelcomePanelProps> = ({
  onQuickAction,
  className = '',
  sessionMode,
}) => {
  const { t } = useTranslation('flow-chat');
  const [gitState, setGitState] = useState<GitWorkState | null>(null);
  const [workspaceDropdownOpen, setWorkspaceDropdownOpen] = useState(false);
  const [isSelectingWorkspace, setIsSelectingWorkspace] = useState(false);
  const workspaceDropdownRef = useRef<HTMLDivElement>(null);

  const { switchLeftPanelTab } = useApp();
  const {
    hasWorkspace,
    currentWorkspace,
    recentWorkspaces,
    openWorkspace,
    switchWorkspace,
  } = useWorkspaceContext();
  const isCoworkSession = (sessionMode || '').toLowerCase() === 'cowork';

  const greeting = useMemo(() => {
    const hour = new Date().getHours();
    if (hour >= 5 && hour < 12) return { title: t('welcome.greetingMorning'), subtitle: t('welcome.subtitleMorning') };
    if (hour >= 12 && hour < 18) return { title: t('welcome.greetingAfternoon'), subtitle: t('welcome.subtitleAfternoon') };
    if (hour >= 18 && hour < 23) return { title: t('welcome.greetingEvening'), subtitle: t('welcome.subtitleEvening') };
    return { title: t('welcome.greetingNight'), subtitle: t('welcome.subtitleNight') };
  }, [t]);

  const tagline = greeting.subtitle;

  const otherWorkspaces = useMemo(
    () => recentWorkspaces.filter(ws => ws.id !== currentWorkspace?.id).slice(0, 6),
    [recentWorkspaces, currentWorkspace?.id],
  );

  const handleGitClick = useCallback(() => {
    switchLeftPanelTab('git');
  }, [switchLeftPanelTab]);

  const isGitClean = useMemo(
    () => !!gitState && gitState.unstagedFiles === 0 && gitState.stagedFiles === 0 && gitState.unpushedCommits === 0,
    [gitState],
  );

  const buildGitNarrative = useCallback((): React.ReactNode => {
    if (!gitState) return null;
    const parts: { key: string; label: string; suffix: string }[] = [];
    if (gitState.unstagedFiles > 0)
      parts.push({ key: 'unstaged', label: `${gitState.unstagedFiles} 个文件`, suffix: '等待暂存' });
    if (gitState.stagedFiles > 0)
      parts.push({ key: 'staged', label: `${gitState.stagedFiles} 个文件`, suffix: '已暂存待提交' });
    if (gitState.unpushedCommits > 0)
      parts.push({ key: 'unpushed', label: `${gitState.unpushedCommits} 个提交`, suffix: '待推送' });
    if (parts.length === 0) return null;
    return (
      <>
        目前有{' '}
        {parts.map(({ key, label, suffix }, i) => (
          <React.Fragment key={key}>
            {i > 0 && '，'}
            <button type="button" className="welcome-panel__inline-btn" onClick={handleGitClick}>
              {label}
            </button>
            {' '}{suffix}
          </React.Fragment>
        ))}
        。
      </>
    );
  }, [gitState, handleGitClick]);

  const loadGitState = useCallback(async (workspacePath: string) => {
    try {
      const isGitRepo = await gitAPI.isGitRepository(workspacePath);
      if (!isGitRepo) { setGitState(null); return; }
      const s = await gitAPI.getStatus(workspacePath);
      setGitState({
        currentBranch: s.current_branch,
        unstagedFiles: s.unstaged.length + s.untracked.length,
        stagedFiles: s.staged.length,
        unpushedCommits: s.ahead,
        aheadBehind: { ahead: s.ahead, behind: s.behind },
        modifiedFiles: [],
      });
    } catch (err) {
      log.warn('Failed to load git state', err);
      setGitState(null);
    }
  }, []);

  useEffect(() => {
    if (isCoworkSession || !currentWorkspace?.rootPath) { setGitState(null); return; }
    void loadGitState(currentWorkspace.rootPath);
  }, [currentWorkspace?.rootPath, isCoworkSession, loadGitState]);

  useEffect(() => {
    if (!workspaceDropdownOpen) return;
    const handler = (e: MouseEvent) => {
      if (workspaceDropdownRef.current && !workspaceDropdownRef.current.contains(e.target as Node)) {
        setWorkspaceDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [workspaceDropdownOpen]);

  const handleSwitchWorkspace = useCallback(async (ws: WorkspaceInfo) => {
    try { setWorkspaceDropdownOpen(false); await switchWorkspace(ws); }
    catch (err) { log.warn('Failed to switch workspace', err); }
  }, [switchWorkspace]);

  const handleOpenOtherFolder = useCallback(async () => {
    try {
      setWorkspaceDropdownOpen(false);
      setIsSelectingWorkspace(true);
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') await openWorkspace(selected);
    } catch (err) {
      log.warn('Failed to open workspace folder', err);
    } finally {
      setIsSelectingWorkspace(false);
    }
  }, [openWorkspace]);

  const handleQuickActionClick = useCallback((cmd: string) => {
    onQuickAction?.(cmd);
  }, [onQuickAction]);

  return (
    <div className={`welcome-panel ${className}`}>
      <div className="welcome-panel__content">
        {/* Greeting */}
        <div className="welcome-panel__greeting">
          <h1 className="welcome-panel__heading">{greeting.title}，{t('welcome.aiPartner')}</h1>
          <p className="welcome-panel__tagline">{tagline}</p>
        </div>

        <div className="welcome-panel__divider" />

        {/* Narrative: workspace + git in natural language */}
        <div className="welcome-panel__narrative" ref={workspaceDropdownRef}>
          <p className="welcome-panel__narrative-text">
            {!hasWorkspace ? (
              <>
                还没有选择项目，
                <button
                  type="button"
                  className="welcome-panel__inline-btn"
                  onClick={() => { void handleOpenOtherFolder(); }}
                  disabled={isSelectingWorkspace}
                >
                  打开一个
                </button>
                {' '}开始吧。
              </>
            ) : (
              <>
                我们正在{' '}
                <button
                  type="button"
                  className={`welcome-panel__inline-btn${workspaceDropdownOpen ? ' welcome-panel__inline-btn--active' : ''}`}
                  onClick={() => setWorkspaceDropdownOpen(v => !v)}
                  disabled={isSelectingWorkspace}
                  title={currentWorkspace?.rootPath}
                >
                  <FolderOpen size={13} className="welcome-panel__inline-icon" />
                  {currentWorkspace?.name || t('welcome.workspace')}
                  <ChevronDown
                    size={11}
                    className={`welcome-panel__inline-chevron${workspaceDropdownOpen ? ' welcome-panel__inline-chevron--open' : ''}`}
                  />
                </button>
                {!isCoworkSession && gitState ? (
                  <>
                    {' '}的{' '}
                    <button type="button" className="welcome-panel__inline-btn" onClick={handleGitClick}>
                      <GitBranch size={13} className="welcome-panel__inline-icon" />
                      {gitState.currentBranch}
                    </button>
                    {' '}分支工作。
                    <br />
                    {isGitClean
                      ? <span className="welcome-panel__narrative-clean">一切干净，可以放手大干了 ✦</span>
                      : buildGitNarrative()
                    }
                  </>
                ) : (
                  <>{' '}项目里工作。</>
                )}
              </>
            )}
          </p>

          {workspaceDropdownOpen && (
            <div className="welcome-panel__dropdown">
              {hasWorkspace && currentWorkspace && (
                <div className="welcome-panel__dropdown-current">
                  <Check size={11} />
                  <FolderOpen size={12} />
                  <span className="welcome-panel__dropdown-name">{currentWorkspace.name}</span>
                </div>
              )}
              {otherWorkspaces.length > 0 && (
                <>
                  {hasWorkspace && <div className="welcome-panel__dropdown-sep" />}
                  {otherWorkspaces.map(ws => (
                    <button
                      key={ws.id}
                      type="button"
                      className="welcome-panel__dropdown-item"
                      onClick={() => { void handleSwitchWorkspace(ws); }}
                      title={ws.rootPath}
                    >
                      <FolderOpen size={12} />
                      <span className="welcome-panel__dropdown-name">{ws.name}</span>
                    </button>
                  ))}
                </>
              )}
              <div className="welcome-panel__dropdown-sep" />
              <button
                type="button"
                className="welcome-panel__dropdown-item welcome-panel__dropdown-item--accent"
                onClick={() => { void handleOpenOtherFolder(); }}
                disabled={isSelectingWorkspace}
              >
                <FolderOpen size={12} />
                <span className="welcome-panel__dropdown-name">{t('welcome.openOtherProject')}</span>
              </button>
            </div>
          )}
        </div>

        {/* Cowork examples */}
        {isCoworkSession && (
          <div className="welcome-panel__cowork">
            <CoworkExampleCards resetKey={0} onSelectPrompt={p => handleQuickActionClick(p)} />
          </div>
        )}
      </div>
    </div>
  );
};

export default WelcomePanel;

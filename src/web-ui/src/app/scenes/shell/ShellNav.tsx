import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Check,
  ChevronDown,
  ChevronRight,
  GitBranch,
  Plus,
  RefreshCw,
  Square,
  SquareTerminal,
  Play,
  Pencil,
  Trash2,
  Pin,
  X,
} from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { BranchSelectModal, type BranchSelectResult } from '@/app/components/panels/BranchSelectModal';
import TerminalEditModal from '@/app/components/panels/TerminalEditModal';
import { useContextMenuStore } from '@/shared/context-menu-system';
import { ContextType } from '@/shared/context-menu-system/types/context.types';
import type { MenuItem } from '@/shared/context-menu-system/types/menu.types';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useTerminalSceneStore } from '@/app/stores/terminalSceneStore';
import type { GitWorktreeInfo } from '@/infrastructure/api/service-api/GitAPI';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { WorkspaceKind } from '@/shared/types';
import { useShellStore } from './shellStore';
import { useShellEntries, useWorktrees, type ShellEntry } from './hooks';
import './ShellNav.scss';

const ShellNav: React.FC = () => {
  // #region agent log
  console.error('[DBG-366fda] ShellNav render v2');
  // #endregion
  const { t } = useI18n('common');
  const { t: tTerminal } = useI18n('panels/terminal');
  const { activeWorkspace, openedWorkspacesList, workspaceName, setActiveWorkspace } = useWorkspaceContext();
  const navView = useShellStore((s) => s.navView);
  const setNavView = useShellStore((s) => s.setNavView);
  const expandedWorktrees = useShellStore((s) => s.expandedWorktrees);
  const toggleWorktree = useShellStore((s) => s.toggleWorktree);
  const activeSceneId = useSceneStore((s) => s.activeTabId);
  const activeTerminalSessionId = useTerminalSceneStore((s) => s.activeSessionId);
  const showMenu = useContextMenuStore((s) => s.showMenu);

  const {
    mainEntries,
    hubMainEntries,
    getWorktreeEntries,
    editModalOpen,
    editingTerminal,
    closeEditModal,
    refresh: refreshEntries,
    createAdHocTerminal,
    createHubTerminal,
    promoteToHub,
    openTerminal,
    stopTerminal,
    deleteTerminal,
    openEditModal,
    saveEdit,
    closeWorktreeTerminals,
    removeWorktreeConfig,
  } = useShellEntries();
  const {
    workspacePath,
    isGitRepo,
    currentBranch,
    worktrees,
    nonMainWorktrees,
    refresh: refreshWorktrees,
    addWorktree,
    removeWorktree,
  } = useWorktrees();

  const [menuOpen, setMenuOpen] = useState(false);
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [branchModalOpen, setBranchModalOpen] = useState(false);
  const [workspaceMenuPosition, setWorkspaceMenuPosition] = useState<{ top: number; left: number } | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const workspaceMenuRef = useRef<HTMLDivElement | null>(null);
  const workspaceTriggerRef = useRef<HTMLButtonElement | null>(null);

  const visibleMainEntries = navView === 'hub' ? hubMainEntries : mainEntries;
  const visibleWorktrees = useMemo(
    () =>
      navView === 'hub'
        ? nonMainWorktrees
        : nonMainWorktrees.filter((worktree) => getWorktreeEntries(worktree.path).length > 0),
    [getWorktreeEntries, navView, nonMainWorktrees],
  );

  const hasVisibleContent = visibleMainEntries.length > 0 || visibleWorktrees.length > 0;

  useEffect(() => {
    if (!menuOpen && !workspaceMenuOpen) {
      return;
    }

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (
        target &&
        (menuRef.current?.contains(target) ||
          workspaceMenuRef.current?.contains(target) ||
          workspaceTriggerRef.current?.contains(target))
      ) {
        return;
      }
      setMenuOpen(false);
      setWorkspaceMenuOpen(false);
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setMenuOpen(false);
        setWorkspaceMenuOpen(false);
      }
    };

    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [menuOpen, workspaceMenuOpen]);

  const hasMultipleWorkspaces = openedWorkspacesList.length > 1;

  useEffect(() => {
    if (!hasMultipleWorkspaces && workspaceMenuOpen) {
      setWorkspaceMenuOpen(false);
    }
  }, [hasMultipleWorkspaces, workspaceMenuOpen]);

  const updateWorkspaceMenuPosition = useCallback(() => {
    const trigger = workspaceTriggerRef.current;
    if (!trigger) {
      return;
    }

    const rect = trigger.getBoundingClientRect();
    const viewportPadding = 8;
    const estimatedWidth = 220;
    const maxLeft = window.innerWidth - estimatedWidth - viewportPadding;

    setWorkspaceMenuPosition({
      top: Math.max(viewportPadding, rect.bottom + 6),
      left: Math.max(viewportPadding, Math.min(rect.left, maxLeft)),
    });
  }, []);

  useEffect(() => {
    if (!workspaceMenuOpen) {
      return;
    }

    updateWorkspaceMenuPosition();

    const handleViewportChange = () => updateWorkspaceMenuPosition();
    window.addEventListener('resize', handleViewportChange);
    window.addEventListener('scroll', handleViewportChange, true);
    return () => {
      window.removeEventListener('resize', handleViewportChange);
      window.removeEventListener('scroll', handleViewportChange, true);
    };
  }, [updateWorkspaceMenuPosition, workspaceMenuOpen]);

  const getWorkspaceDisplayName = useCallback(
    (workspace: (typeof openedWorkspacesList)[number]) =>
      workspace.workspaceKind === WorkspaceKind.Assistant
        ? workspace.identity?.name?.trim() || workspace.name
        : workspace.name,
    [],
  );

  const handleRefresh = useCallback(async () => {
    await Promise.all([refreshEntries(), refreshWorktrees()]);
  }, [refreshEntries, refreshWorktrees]);

  const handleCreateAdHocTerminal = useCallback(async () => {
    setMenuOpen(false);
    await createAdHocTerminal();
  }, [createAdHocTerminal]);

  const handleCreateHubTerminal = useCallback(async () => {
    setMenuOpen(false);
    await createHubTerminal();
  }, [createHubTerminal]);

  const handleToggleCreateMenu = useCallback(() => {
    setWorkspaceMenuOpen(false);
    setMenuOpen((prev) => !prev);
  }, []);

  const handleToggleWorkspaceMenu = useCallback(() => {
    if (!hasMultipleWorkspaces) {
      return;
    }

    setMenuOpen(false);
    setWorkspaceMenuOpen((prev) => !prev);
  }, [hasMultipleWorkspaces]);

  const handleSelectWorkspace = useCallback(async (workspaceId: string) => {
    setWorkspaceMenuOpen(false);
    if (workspaceId === activeWorkspace?.id) {
      return;
    }
    await setActiveWorkspace(workspaceId);
  }, [activeWorkspace?.id, setActiveWorkspace]);

  const handleOpenBranchModal = useCallback(() => {
    setMenuOpen(false);
    setBranchModalOpen(true);
  }, []);

  const handleBranchSelect = useCallback(async (result: BranchSelectResult) => {
    await addWorktree(result.branch, result.isNew);
    setNavView('hub');
  }, [addWorktree, setNavView]);

  const handleRemoveWorktree = useCallback(async (worktree: GitWorktreeInfo) => {
    const confirmed = window.confirm(tTerminal('dialog.deleteWorktree.message'));
    if (!confirmed) {
      return;
    }

    await closeWorktreeTerminals(worktree.path);
    const removed = await removeWorktree(worktree.path);
    if (removed) {
      removeWorktreeConfig(worktree.path);
    }
  }, [closeWorktreeTerminals, removeWorktree, removeWorktreeConfig, tTerminal]);

  const openContextMenu = useCallback((
    event: React.MouseEvent<HTMLElement>,
    items: MenuItem[],
    data: Record<string, unknown>,
  ) => {
    event.preventDefault();
    event.stopPropagation();

    showMenu(
      { x: event.clientX, y: event.clientY },
      items,
      {
        type: ContextType.CUSTOM,
        customType: 'shell-nav',
        data,
        event,
        targetElement: event.currentTarget,
        position: { x: event.clientX, y: event.clientY },
        timestamp: Date.now(),
      },
    );
  }, [showMenu]);

  const getEntryMenuItems = useCallback((entry: ShellEntry): MenuItem[] => {
    if (entry.isHub) {
      return [
        !entry.isRunning
          ? {
              id: `start-${entry.sessionId}`,
              label: t('nav.shell.context.start'),
              icon: <Play size={14} />,
              onClick: async () => {
                // #region agent log
                console.error('[DBG-366fda][H-A] Start menu clicked', {sessionId: entry.sessionId, isHub: entry.isHub, isRunning: entry.isRunning});
                // #endregion
                await openTerminal(entry);
              },
            }
          : {
              id: `stop-${entry.sessionId}`,
              label: t('nav.shell.context.stop'),
              icon: <Square size={14} />,
              onClick: async () => {
                await stopTerminal(entry.sessionId);
              },
            },
        {
          id: `edit-${entry.sessionId}`,
          label: t('nav.shell.context.editConfig'),
          icon: <Pencil size={14} />,
          onClick: () => {
            openEditModal(entry);
          },
        },
        {
          id: `delete-${entry.sessionId}`,
          label: t('nav.shell.context.delete'),
          icon: <Trash2 size={14} />,
          onClick: async () => {
            await deleteTerminal(entry);
          },
        },
      ];
    }

    return [
      {
        id: `rename-${entry.sessionId}`,
        label: t('nav.shell.context.rename'),
        icon: <Pencil size={14} />,
        onClick: () => {
          openEditModal(entry);
        },
      },
      {
        id: `promote-${entry.sessionId}`,
        label: t('nav.shell.context.promoteToHub'),
        icon: <Pin size={14} />,
        onClick: () => {
          promoteToHub(entry);
        },
      },
      {
        id: `close-${entry.sessionId}`,
        label: t('nav.shell.context.close'),
        icon: <X size={14} />,
        onClick: async () => {
          await deleteTerminal(entry);
        },
      },
    ];
  }, [deleteTerminal, openEditModal, openTerminal, promoteToHub, stopTerminal, t]);

  const getWorktreeMenuItems = useCallback((worktree: GitWorktreeInfo): MenuItem[] => [
    {
      id: `create-${worktree.path}`,
      label: t('nav.shell.context.newWorktreeTerminal'),
      icon: <Plus size={14} />,
      onClick: async () => {
        await createHubTerminal(worktree.path);
      },
    },
    {
      id: `remove-${worktree.path}`,
      label: t('nav.shell.context.removeWorktree'),
      icon: <Trash2 size={14} />,
      onClick: async () => {
        await handleRemoveWorktree(worktree);
      },
    },
  ], [createHubTerminal, handleRemoveWorktree, t]);

  const renderTerminalEntry = useCallback((entry: ShellEntry) => {
    const isActive = activeSceneId === 'shell' && activeTerminalSessionId === entry.sessionId;

    return (
      <div
        key={entry.sessionId}
        role="button"
        tabIndex={0}
        className={[
          'bitfun-shell-nav__terminal-item',
          isActive && 'is-active',
        ].filter(Boolean).join(' ')}
        onClick={() => { void openTerminal(entry); }}
        onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { void openTerminal(entry); } }}
        onContextMenu={(event) => openContextMenu(event, getEntryMenuItems(entry), { entry })}
        title={entry.name}
      >
        <SquareTerminal size={14} className="bitfun-shell-nav__terminal-icon" />
        <span className="bitfun-shell-nav__terminal-label">{entry.name}</span>
        {navView === 'hub' && entry.startupCommand ? (
          <span className="bitfun-shell-nav__cmd-indicator">{t('nav.shell.badges.startupCommand')}</span>
        ) : null}
        <span className={`bitfun-shell-nav__terminal-dot${entry.isRunning ? ' is-running' : ' is-stopped'}`} />
        <button
          type="button"
          className="bitfun-shell-nav__terminal-close"
          onClick={(e) => { e.stopPropagation(); void deleteTerminal(entry); }}
          title={t('nav.shell.context.close')}
        >
          <X size={12} />
        </button>
      </div>
    );
  }, [activeSceneId, activeTerminalSessionId, deleteTerminal, getEntryMenuItems, navView, openContextMenu, openTerminal, t]);

  return (
    <div className="bitfun-shell-nav">
      <div className="bitfun-shell-nav__header">
        <div className="bitfun-shell-nav__title-group">
          <span className="bitfun-shell-nav__title">{t('nav.shell.title')}</span>
          {workspaceName ? (
            <div className="bitfun-shell-nav__workspace-switcher">
              <button
                ref={workspaceTriggerRef}
                type="button"
                className={`bitfun-shell-nav__workspace-trigger${workspaceMenuOpen ? ' is-active' : ''}${hasMultipleWorkspaces ? ' is-switchable' : ''}`}
                onClick={handleToggleWorkspaceMenu}
                aria-haspopup={hasMultipleWorkspaces ? 'menu' : undefined}
                aria-expanded={hasMultipleWorkspaces ? workspaceMenuOpen : undefined}
                title={hasMultipleWorkspaces ? t('header.switchWorkspace') : workspaceName}
              >
                <span className="bitfun-shell-nav__workspace-separator">/</span>
                <span className="bitfun-shell-nav__workspace-name">{workspaceName}</span>
                {hasMultipleWorkspaces ? (
                  <ChevronDown size={12} className="bitfun-shell-nav__workspace-trigger-icon" />
                ) : null}
              </button>

              {workspaceMenuOpen && hasMultipleWorkspaces ? (
                workspaceMenuPosition ? createPortal(
                  <div
                    ref={workspaceMenuRef}
                    className="bitfun-shell-nav__workspace-menu"
                    role="menu"
                    aria-label={t('header.switchWorkspace')}
                    style={{
                      top: `${workspaceMenuPosition.top}px`,
                      left: `${workspaceMenuPosition.left}px`,
                    }}
                  >
                    {openedWorkspacesList.map((workspace) => {
                      const isActive = workspace.id === activeWorkspace?.id;
                      const label = getWorkspaceDisplayName(workspace);

                      return (
                        <button
                          key={workspace.id}
                          type="button"
                          role="menuitemradio"
                          aria-checked={isActive}
                          className={`bitfun-shell-nav__workspace-menu-item${isActive ? ' is-active' : ''}`}
                          onClick={() => { void handleSelectWorkspace(workspace.id); }}
                          title={workspace.rootPath}
                        >
                          <span className="bitfun-shell-nav__workspace-menu-check" aria-hidden="true">
                            {isActive ? <Check size={12} /> : null}
                          </span>
                          <span className="bitfun-shell-nav__workspace-menu-text">{label}</span>
                        </button>
                      );
                    })}
                  </div>,
                  document.body,
                ) : null
              ) : null}
            </div>
          ) : null}
        </div>
        <div className="bitfun-shell-nav__header-actions" ref={menuRef}>
          <button
            type="button"
            className={`bitfun-shell-nav__menu-trigger${menuOpen ? ' is-active' : ''}`}
            onClick={handleToggleCreateMenu}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            title={t('actions.new')}
          >
            <Plus size={14} />
          </button>

          {menuOpen ? (
            <div className="bitfun-shell-nav__dropdown-menu" role="menu">
              <button type="button" className="bitfun-shell-nav__dropdown-item" role="menuitem" onClick={() => { void handleCreateAdHocTerminal(); }}>
                <SquareTerminal size={14} />
                <span>{t('nav.shell.actions.newTerminal')}</span>
              </button>
              <button type="button" className="bitfun-shell-nav__dropdown-item" role="menuitem" onClick={() => { void handleCreateHubTerminal(); }}>
                <Pin size={14} />
                <span>{t('nav.shell.actions.newHubTerminal')}</span>
              </button>
              {isGitRepo ? (
                <button type="button" className="bitfun-shell-nav__dropdown-item" role="menuitem" onClick={handleOpenBranchModal}>
                  <GitBranch size={14} />
                  <span>{t('nav.shell.actions.addWorktree')}</span>
                </button>
              ) : null}
              <button type="button" className="bitfun-shell-nav__dropdown-item" role="menuitem" onClick={() => { setMenuOpen(false); void handleRefresh(); }}>
                <RefreshCw size={14} />
                <span>{t('nav.shell.actions.refresh')}</span>
              </button>
            </div>
          ) : null}
        </div>
      </div>

      <div className="bitfun-shell-nav__view-toggle" role="tablist" aria-label={t('nav.shell.title')}>
        <button
          type="button"
          role="tab"
          className={`bitfun-shell-nav__view-toggle-btn${navView === 'all' ? ' is-active' : ''}`}
          aria-selected={navView === 'all'}
          onClick={() => setNavView('all')}
        >
          {t('nav.shell.views.all')}
        </button>
        <button
          type="button"
          role="tab"
          className={`bitfun-shell-nav__view-toggle-btn${navView === 'hub' ? ' is-active' : ''}`}
          aria-selected={navView === 'hub'}
          onClick={() => setNavView('hub')}
        >
          {t('nav.shell.views.hub')}
        </button>
      </div>

      <div className="bitfun-shell-nav__sections">
        {hasVisibleContent ? (
          <>
            {visibleMainEntries.length > 0 ? (
              <div className="bitfun-shell-nav__terminal-list">
                {visibleMainEntries.map((entry) => renderTerminalEntry(entry))}
              </div>
            ) : null}

            {visibleWorktrees.map((worktree) => {
              const entries = getWorktreeEntries(worktree.path);
              const expanded = expandedWorktrees.has(worktree.path);
              const branchLabel = worktree.branch || worktree.path.split(/[/\\]/).pop() || worktree.path;

              return (
                <div key={worktree.path} className="bitfun-shell-nav__worktree-group">
                  <div
                    role="button"
                    tabIndex={0}
                    className="bitfun-shell-nav__worktree-header"
                    onClick={() => toggleWorktree(worktree.path)}
                    onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { toggleWorktree(worktree.path); } }}
                    onContextMenu={(event) => openContextMenu(event, getWorktreeMenuItems(worktree), { worktree })}
                  >
                    <ChevronRight
                      size={12}
                      className={`bitfun-shell-nav__worktree-chevron${expanded ? ' is-expanded' : ''}`}
                    />
                    <GitBranch size={13} className="bitfun-shell-nav__worktree-icon" />
                    <span className="bitfun-shell-nav__worktree-label">{branchLabel}</span>
                    <span className="bitfun-shell-nav__worktree-count">{entries.length}</span>
                    <button
                      type="button"
                      className="bitfun-shell-nav__worktree-add"
                      onClick={(e) => { e.stopPropagation(); void createHubTerminal(worktree.path); }}
                      title={t('nav.shell.context.newWorktreeTerminal')}
                    >
                      <Plus size={12} />
                    </button>
                  </div>

                  {expanded ? (
                    <div className="bitfun-shell-nav__worktree-list">
                      {entries.length > 0 ? (
                        entries.map((entry) => renderTerminalEntry(entry))
                      ) : navView === 'hub' ? (
                        <div className="bitfun-shell-nav__empty bitfun-shell-nav__empty--nested">
                          {t('nav.shell.empty.hub')}
                        </div>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              );
            })}
          </>
        ) : (
          <div className="bitfun-shell-nav__empty">
            {navView === 'hub' ? t('nav.shell.empty.hub') : t('nav.shell.empty.all')}
          </div>
        )}
      </div>

      {workspacePath ? (
        <BranchSelectModal
          isOpen={branchModalOpen}
          onClose={() => setBranchModalOpen(false)}
          onSelect={(result) => { void handleBranchSelect(result); }}
          repositoryPath={workspacePath}
          currentBranch={currentBranch}
          existingWorktreeBranches={worktrees.map((worktree) => worktree.branch).filter(Boolean) as string[]}
          title={t('nav.shell.actions.addWorktree')}
        />
      ) : null}

      {editingTerminal ? (
        <TerminalEditModal
          isOpen={editModalOpen}
          onClose={closeEditModal}
          onSave={saveEdit}
          initialName={editingTerminal.terminal.name}
          initialStartupCommand={editingTerminal.terminal.startupCommand}
          showStartupCommand={editingTerminal.isHub}
        />
      ) : null}
    </div>
  );
};

export default ShellNav;

/**
 * Terminal Sessions Panel
 * Displays all active terminal sessions with Terminal Hub support
 * - Worktree management
 * - Terminal session persistence
 * - Lazy loading startup
 * - Rename functionality
 */

import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Terminal as TerminalIcon,
  Plus,
  Trash2,
  Play,
  Square,
  ChevronRight,
  Monitor,
  Layers,
  RefreshCw,
  GitBranch,
  Edit2
} from 'lucide-react';
import { getTerminalService, type TerminalService } from '../../../tools/terminal';
import type { SessionResponse } from '../../../tools/terminal/types/session';
import { Tooltip } from '@/component-library';
import { createTerminalTab } from '../../../shared/utils/tabUtils';
import { useCurrentWorkspace } from '../../../infrastructure/contexts/WorkspaceContext';
import { configManager } from '../../../infrastructure/config/services/ConfigManager';
import type { TerminalConfig } from '../../../infrastructure/config/types';
import { gitAPI, type GitWorktreeInfo } from '../../../infrastructure/api/service-api/GitAPI';
import { BranchSelectModal, type BranchSelectResult } from './BranchSelectModal';
import { TerminalEditModal } from './TerminalEditModal';
import { PanelHeader } from './base';
import { IconButton } from '../../../component-library';
import { useNotification } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './TerminalSessionsPanel.scss';

const log = createLogger('TerminalSessionsPanel');

// ==================== Type Definitions ====================

interface TerminalEntry {
  sessionId: string;
  name: string;
  startupCommand?: string;
}

interface TerminalHubConfig {
  terminals: TerminalEntry[];
  worktrees: Record<string, TerminalEntry[]>;
}

const TERMINAL_HUB_STORAGE_KEY = 'bitfun-terminal-hub-config';
const HUB_TERMINAL_ID_PREFIX = 'hub_';

// ==================== Utility Functions ====================
const generateHubTerminalId = () => `${HUB_TERMINAL_ID_PREFIX}${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

const loadHubConfig = (workspacePath: string): TerminalHubConfig => {
  try {
    const key = `${TERMINAL_HUB_STORAGE_KEY}:${workspacePath}`;
    const saved = localStorage.getItem(key);
    if (saved) {
      return JSON.parse(saved);
    }
  } catch (error) {
    log.error('Failed to load hub config', error);
  }
  return { terminals: [], worktrees: {} };
};

const saveHubConfig = (workspacePath: string, config: TerminalHubConfig) => {
  try {
    const key = `${TERMINAL_HUB_STORAGE_KEY}:${workspacePath}`;
    localStorage.setItem(key, JSON.stringify(config));
  } catch (error) {
    log.error('Failed to save hub config', error);
  }
};

// ==================== Component Props ====================

interface TerminalSessionsPanelProps {
  className?: string;
}

// ==================== Main Component ====================

const TerminalSessionsPanel: React.FC<TerminalSessionsPanelProps> = ({
  className = ''
}) => {
  const { t } = useTranslation('panels/terminal');
  
  // ==================== State ====================
  const [sessions, setSessions] = useState<SessionResponse[]>([]);
  const [terminalsExpanded, setTerminalsExpanded] = useState(true);
  const [hubExpanded, setHubExpanded] = useState(true);
  const { workspacePath } = useCurrentWorkspace();
  const notification = useNotification();

  const [hubConfig, setHubConfig] = useState<TerminalHubConfig>({ terminals: [], worktrees: {} });
  const [worktrees, setWorktrees] = useState<GitWorktreeInfo[]>([]);
  const [expandedWorktrees, setExpandedWorktrees] = useState<Set<string>>(new Set());
  const [isGitRepo, setIsGitRepo] = useState(false);
  const [branchModalOpen, setBranchModalOpen] = useState(false);
  const [currentBranch, setCurrentBranch] = useState<string | undefined>();

  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editingTerminal, setEditingTerminal] = useState<{
    terminal: TerminalEntry;
    worktreePath?: string;
  } | null>(null);

  const terminalServiceRef = useRef<TerminalService | null>(null);

  const activeSessionIds = useMemo(
    () => new Set(sessions.map(s => s.id)),
    [sessions]
  );

  const isTerminalRunning = useCallback(
    (sessionId: string) => activeSessionIds.has(sessionId),
    [activeSessionIds]
  );

  const [pendingDeleteWorktree, setPendingDeleteWorktree] = useState<string | null>(null);

  // ==================== Initialization ====================

  const loadSessions = useCallback(async () => {
    const service = terminalServiceRef.current;
    if (!service) return;

    try {
      const sessionList = await service.listSessions();
      setSessions(sessionList);
    } catch (err) {
      log.error('Failed to load sessions', err);
      notification.error(t('notifications.loadSessionsFailed'));
    }
  }, [t]);

  useEffect(() => {
    const service = getTerminalService();
    terminalServiceRef.current = service;

    const init = async () => {
      try {
        await service.connect();
        await loadSessions();
      } catch (err) {
        log.error('Failed to connect terminal service', err);
        notification.error(t('notifications.connectFailed'));
      }
    };

    init();

    const unsubscribe = service.onEvent((event) => {
      if (event.type === 'ready' || event.type === 'exit') {
        if (event.type === 'exit' && event.sessionId) {
          setSessions(prev => {
            const exists = prev.some(s => s.id === event.sessionId);
            if (exists) {
              window.dispatchEvent(new CustomEvent('terminal-session-destroyed', {
                detail: { sessionId: event.sessionId }
              }));
            }
            return prev.filter(s => s.id !== event.sessionId);
          });
        }
        loadSessions();
      }
    });

    return () => {
      unsubscribe();
    };
  }, [loadSessions]);

  useEffect(() => {
    if (!workspacePath) return;
    
    const config = loadHubConfig(workspacePath);
    setHubConfig(config);
    checkGitAndLoadWorktrees();
  }, [workspacePath]);

  useEffect(() => {
    const handleCreateHubTerminal = async (event: CustomEvent<{ name: string; startupCommand: string; worktreePath?: string }>) => {
      const service = terminalServiceRef.current;
      if (!workspacePath || !service) return;
      
      const { name, startupCommand, worktreePath } = event.detail;
      
      const newTerminal: TerminalEntry = {
        sessionId: generateHubTerminalId(),
        name,
        startupCommand,
      };
      
      setHubConfig(prev => {
        let newConfig: TerminalHubConfig;
        
        if (worktreePath) {
          const existing = prev.worktrees[worktreePath] || [];
          newConfig = {
            ...prev,
            worktrees: {
              ...prev.worktrees,
              [worktreePath]: [...existing, newTerminal]
            }
          };
        } else {
          newConfig = {
            ...prev,
            terminals: [...prev.terminals, newTerminal]
          };
        }
        
        saveHubConfig(workspacePath, newConfig);
        return newConfig;
      });
      
      try {
        let shellType: string | undefined;
        try {
          const terminalConfig = await configManager.getConfig<TerminalConfig>('terminal');
          if (terminalConfig?.default_shell) {
            shellType = terminalConfig.default_shell;
          }
        } catch (configErr) {
          log.warn('Failed to read terminal config', configErr);
        }
        
        const cwd = worktreePath || workspacePath;
        const createRequest = {
          workingDirectory: cwd,
          name: newTerminal.name,
          shellType,
          sessionId: newTerminal.sessionId,
        };
        
        await service.createSession(createRequest);
        createTerminalTab(newTerminal.sessionId, newTerminal.name);
        loadSessions();
        
        if (startupCommand?.trim()) {
          try {
            await service.sendCommand(newTerminal.sessionId, startupCommand);
          } catch (cmdErr) {
            log.warn('Failed to execute startup command', cmdErr);
          }
        }
      } catch (err) {
        log.error('Failed to create external terminal', err);
        notification.error(t('notifications.createFailed'));
      }
    };
    
    window.addEventListener('create-hub-terminal', handleCreateHubTerminal as unknown as EventListener);
    
    return () => {
      window.removeEventListener('create-hub-terminal', handleCreateHubTerminal as unknown as EventListener);
    };
  }, [workspacePath, loadSessions, notification]);

  // ==================== Git/Worktree Operations ====================

  const checkGitAndLoadWorktrees = useCallback(async () => {
    if (!workspacePath) return;
    
    try {
      const isRepo = await gitAPI.isGitRepository(workspacePath);
      setIsGitRepo(isRepo);
      
      if (isRepo) {
        await refreshWorktrees();
      }
    } catch (err) {
      log.error('Failed to check git repository', err);
      setIsGitRepo(false);
    }
  }, [workspacePath]);

  const refreshWorktrees = useCallback(async () => {
    if (!workspacePath) return;

    try {
      const wtList = await gitAPI.listWorktrees(workspacePath);
      setWorktrees(wtList);

      try {
        const branches = await gitAPI.getBranches(workspacePath, false);
        const current = branches.find(b => b.current);
        setCurrentBranch(current?.name);
      } catch (err) {
        log.warn('Failed to get current branch', err);
        setCurrentBranch(undefined);
      }

      setHubConfig(prev => {
        const existingPaths = new Set(wtList.map(wt => wt.path));
        const newWorktrees: Record<string, TerminalEntry[]> = {};

        for (const [path, terminals] of Object.entries(prev.worktrees)) {
          if (existingPaths.has(path)) {
            newWorktrees[path] = terminals;
          }
        }

        const newConfig = { ...prev, worktrees: newWorktrees };
        saveHubConfig(workspacePath, newConfig);
        return newConfig;
      });
    } catch (err) {
      log.error('Failed to load worktrees', err);
    }
  }, [workspacePath]);

  const handleRefreshHub = useCallback(async () => {
    await checkGitAndLoadWorktrees();
  }, [checkGitAndLoadWorktrees]);

  const handleAddWorktree = useCallback(() => {
    if (!isGitRepo) {
      notification.error(t('notifications.notGitRepo'));
      return;
    }
    setBranchModalOpen(true);
  }, [isGitRepo, t]);

  const handleBranchSelect = useCallback(async (result: BranchSelectResult) => {
    if (!workspacePath) return;

    try {
      await gitAPI.addWorktree(workspacePath, result.branch, result.isNew);
      await refreshWorktrees();
    } catch (err) {
      log.error('Failed to add worktree', err);
      notification.error(t('notifications.addWorktreeFailed', { error: String(err) }));
    }
  }, [workspacePath, refreshWorktrees, t]);

  const handleRemoveWorktree = useCallback((worktreePath: string) => {
    setPendingDeleteWorktree(worktreePath);
  }, []);

  const confirmRemoveWorktree = useCallback(async () => {
    if (!workspacePath || !pendingDeleteWorktree) return;
    
    const worktreePath = pendingDeleteWorktree;
    setPendingDeleteWorktree(null);
    
    const terminals = hubConfig.worktrees[worktreePath] || [];
    const service = terminalServiceRef.current;
    
    let closedCount = 0;
    
    if (service) {
      for (const term of terminals) {
        if (isTerminalRunning(term.sessionId)) {
          try {
            await service.closeSession(term.sessionId);
            closedCount++;
          } catch (err) {
            log.warn('Failed to close terminal', err);
          }
        }
      }
      
      for (const session of sessions) {
        const sessionCwd = session.cwd || '';
        const normalizedWorktree = worktreePath.replace(/\\/g, '/').toLowerCase();
        const normalizedCwd = sessionCwd.replace(/\\/g, '/').toLowerCase();
        
        if (normalizedCwd.startsWith(normalizedWorktree)) {
          try {
            await service.closeSession(session.id);
            closedCount++;
          } catch (err) {
            log.warn('Failed to close session terminal', err);
          }
        }
      }
    }
    
    if (closedCount > 0) {
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    
    const maxRetries = 3;
    const retryDelay = 1000;
    
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        await gitAPI.removeWorktree(workspacePath, worktreePath, true);
        await refreshWorktrees();
        return;
      } catch (err) {
        if (attempt < maxRetries) {
          await new Promise(resolve => setTimeout(resolve, retryDelay));
        } else {
          log.error('Failed to delete worktree', err);
          notification.error(t('notifications.deleteWorktreeFailed'));
        }
      }
    }
  }, [workspacePath, pendingDeleteWorktree, hubConfig, sessions, refreshWorktrees, isTerminalRunning, t]);

  const cancelRemoveWorktree = useCallback(() => {
    setPendingDeleteWorktree(null);
  }, []);

  // ==================== Terminal Operations ====================

  const handleCreateSession = useCallback(async () => {
    const service = terminalServiceRef.current;
    if (!service) return;
    
    try {
      let shellType: string | undefined;
      try {
        const terminalConfig = await configManager.getConfig<TerminalConfig>('terminal');
        if (terminalConfig?.default_shell) {
          shellType = terminalConfig.default_shell;
        }
      } catch (configErr) {
        log.warn('Failed to read terminal config', configErr);
      }
      
      const createRequest = {
        workingDirectory: workspacePath,
        name: `Terminal ${sessions.length + 1}`,
        shellType,
      };
      const session = await service.createSession(createRequest);
      setSessions(prev => [...prev, session]);
      createTerminalTab(session.id, session.name);
      
      setTimeout(() => loadSessions(), 500);
    } catch (err) {
      log.error('Failed to create session', err);
      notification.error(t('notifications.createFailed'));
    }
  }, [workspacePath, sessions.length, loadSessions, t]);

  const handleAddHubTerminal = useCallback(async (worktreePath?: string) => {
    const service = terminalServiceRef.current;
    if (!workspacePath || !service) return;
    
    const newTerminal: TerminalEntry = {
      sessionId: generateHubTerminalId(),
      name: `Terminal ${Date.now() % 1000}`,
    };
    
    setHubConfig(prev => {
      let newConfig: TerminalHubConfig;
      
      if (worktreePath) {
        const existing = prev.worktrees[worktreePath] || [];
        newConfig = {
          ...prev,
          worktrees: {
            ...prev.worktrees,
            [worktreePath]: [...existing, newTerminal]
          }
        };
      } else {
        newConfig = {
          ...prev,
          terminals: [...prev.terminals, newTerminal]
        };
      }
      
      saveHubConfig(workspacePath, newConfig);
      return newConfig;
    });
    
    try {
      let shellType: string | undefined;
      try {
        const terminalConfig = await configManager.getConfig<TerminalConfig>('terminal');
        if (terminalConfig?.default_shell) {
          shellType = terminalConfig.default_shell;
        }
        } catch (configErr) {
          log.warn('Failed to read terminal config', configErr);
        }
      
      const cwd = worktreePath || workspacePath;
      const createRequest = {
        workingDirectory: cwd,
        name: newTerminal.name,
        shellType,
        sessionId: newTerminal.sessionId,
      };
      
      await service.createSession(createRequest);
      createTerminalTab(newTerminal.sessionId, newTerminal.name);
      loadSessions();
    } catch (err) {
      log.error('Failed to auto-start terminal', err);
      notification.error(t('notifications.createFailed'));
    }
  }, [workspacePath, loadSessions, t]);

  const handleStartTerminal = useCallback(async (terminal: TerminalEntry, worktreePath?: string) => {
    const service = terminalServiceRef.current;
    if (!service || !workspacePath) return;
    
    if (isTerminalRunning(terminal.sessionId)) {
      createTerminalTab(terminal.sessionId, terminal.name);
      return;
    }
    
    try {
      let shellType: string | undefined;
      try {
        const terminalConfig = await configManager.getConfig<TerminalConfig>('terminal');
        if (terminalConfig?.default_shell) {
          shellType = terminalConfig.default_shell;
        }
        } catch (configErr) {
          log.warn('Failed to read terminal config', configErr);
        }
      
      const cwd = worktreePath || workspacePath;
      const createRequest = {
        workingDirectory: cwd,
        name: terminal.name,
        shellType,
        sessionId: terminal.sessionId,
      };
      
      await service.createSession(createRequest);
      
      createTerminalTab(terminal.sessionId, terminal.name);
      loadSessions();
      
      if (terminal.startupCommand?.trim()) {
        const waitForResize = new Promise<void>((resolve) => {
          const unsubscribe = service.onSessionEvent(terminal.sessionId, (event) => {
            if (event.type === 'resize') {
              unsubscribe();
              resolve();
            }
          });
          setTimeout(() => {
            unsubscribe();
            resolve();
          }, 5000);
        });
        
        await waitForResize;
        
        try {
          await service.sendCommand(terminal.sessionId, terminal.startupCommand);
        } catch (cmdErr) {
          log.warn('Failed to execute startup command', cmdErr);
        }
      }
    } catch (err) {
      log.error('Failed to start terminal', err);
      notification.error(t('notifications.startFailed'));
    }
  }, [workspacePath, loadSessions, isTerminalRunning, t]);

  const handleStopHubTerminal = useCallback(async (terminal: TerminalEntry, e: React.MouseEvent) => {
    e.stopPropagation();
    
    const service = terminalServiceRef.current;
    if (!service || !isTerminalRunning(terminal.sessionId)) return;
    
    try {
      await service.closeSession(terminal.sessionId);
      window.dispatchEvent(new CustomEvent('terminal-session-destroyed', {
        detail: { sessionId: terminal.sessionId }
      }));
      loadSessions();
    } catch (err) {
      log.error('Failed to stop terminal', err);
      notification.error(t('notifications.stopFailed'));
    }
  }, [isTerminalRunning, loadSessions, t]);

  const handleDeleteHubTerminal = useCallback(async (terminal: TerminalEntry, worktreePath?: string) => {
    const service = terminalServiceRef.current;
    if (!workspacePath) return;
    
    if (isTerminalRunning(terminal.sessionId) && service) {
      try {
        await service.closeSession(terminal.sessionId);
        window.dispatchEvent(new CustomEvent('terminal-session-destroyed', {
      detail: { sessionId: terminal.sessionId }
    }));
      } catch (err) {
        log.warn('Failed to close terminal', err);
      }
    }
    
    setHubConfig(prev => {
      let newConfig: TerminalHubConfig;
      
      if (worktreePath) {
        const terminals = prev.worktrees[worktreePath] || [];
        newConfig = {
          ...prev,
          worktrees: {
            ...prev.worktrees,
            [worktreePath]: terminals.filter(t => t.sessionId !== terminal.sessionId)
          }
        };
      } else {
        newConfig = {
          ...prev,
          terminals: prev.terminals.filter(t => t.sessionId !== terminal.sessionId)
        };
      }
      
      saveHubConfig(workspacePath, newConfig);
      return newConfig;
    });
  }, [workspacePath, isTerminalRunning]);

  const handleOpenSession = useCallback((session: SessionResponse) => {
    createTerminalTab(session.id, session.name);
  }, []);

  const handleCloseSession = useCallback(async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    
    const service = terminalServiceRef.current;
    if (!service) return;
    
    try {
      await service.closeSession(sessionId);
      setSessions(prev => prev.filter(s => s.id !== sessionId));
      
      window.dispatchEvent(new CustomEvent('terminal-session-destroyed', {
        detail: { sessionId }
      }));
    } catch (err) {
      log.error('Failed to close session', err);
    }
  }, []);

  // ==================== Edit Modal Functionality ====================

  const handleOpenEditModal = useCallback((terminal: TerminalEntry, worktreePath: string | undefined, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingTerminal({ terminal, worktreePath });
    setEditModalOpen(true);
  }, []);

  const handleSaveTerminalEdit = useCallback((newName: string, newStartupCommand?: string) => {
    if (!editingTerminal || !workspacePath) return;

    const { terminal, worktreePath } = editingTerminal;

    setHubConfig(prev => {
      let newConfig: TerminalHubConfig;

      if (worktreePath) {
        const terminals = prev.worktrees[worktreePath] || [];
        const updatedTerminals = terminals.map(t =>
          t.sessionId === terminal.sessionId 
            ? { ...t, name: newName, startupCommand: newStartupCommand } 
            : t
        );
        newConfig = {
          ...prev,
          worktrees: {
            ...prev.worktrees,
            [worktreePath]: updatedTerminals
          }
        };
      } else {
        const updatedTerminals = prev.terminals.map(t =>
          t.sessionId === terminal.sessionId 
            ? { ...t, name: newName, startupCommand: newStartupCommand } 
            : t
        );
        newConfig = {
          ...prev,
          terminals: updatedTerminals
        };
      }

      saveHubConfig(workspacePath, newConfig);
      return newConfig;
    });

    if (isTerminalRunning(terminal.sessionId)) {
      setSessions(prev => prev.map(s =>
        s.id === terminal.sessionId ? { ...s, name: newName } : s
      ));

      window.dispatchEvent(new CustomEvent('terminal-session-renamed', {
        detail: { sessionId: terminal.sessionId, newName }
      }));
    }

    setEditingTerminal(null);
  }, [editingTerminal, workspacePath, isTerminalRunning]);

  // ==================== Worktree Expand/Collapse ====================

  const toggleWorktreeExpanded = useCallback((path: string) => {
    setExpandedWorktrees(prev => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  // ==================== Render Terminal Item ====================

  const renderTerminalItem = (terminal: TerminalEntry, worktreePath?: string) => {
    const isRunning = isTerminalRunning(terminal.sessionId);
    
    return (
      <div
        key={terminal.sessionId}
        className={`terminal-sessions-panel__item ${isRunning ? 'running' : 'idle'}`}
        onClick={() => handleStartTerminal(terminal, worktreePath)}
      >
        <div className="terminal-sessions-panel__item-icon">
          <TerminalIcon size={14} />
        </div>
        <div className="terminal-sessions-panel__item-info">
          <div 
            className="terminal-sessions-panel__item-name"
            onDoubleClick={(e) => handleOpenEditModal(terminal, worktreePath, e)}
          >
            {terminal.name}
          </div>
          <div className="terminal-sessions-panel__item-meta">
            <span className="terminal-sessions-panel__item-status">
              {isRunning ? (
                <>
                  <Play size={10} />
                  {t('status.running')}
                </>
              ) : (
                <>
                  <Square size={10} />
                  {t('status.idle')}
                </>
              )}
            </span>
          </div>
        </div>
        <div className="terminal-sessions-panel__item-actions">
          <Tooltip content={t('actions.edit')}>
            <button
              className="terminal-sessions-panel__item-action terminal-sessions-panel__item-action--edit"
              onClick={(e) => handleOpenEditModal(terminal, worktreePath, e)}
            >
              <Edit2 size={12} />
            </button>
          </Tooltip>
          {isRunning && (
            <Tooltip content={t('actions.stopTerminal')}>
              <button
                className="terminal-sessions-panel__item-action terminal-sessions-panel__item-action--stop"
                onClick={(e) => handleStopHubTerminal(terminal, e)}
              >
                <Square size={12} />
              </button>
            </Tooltip>
          )}
          <Tooltip content={t('actions.deleteTerminal')}>
            <button
              className="terminal-sessions-panel__item-action"
              onClick={(e) => {
                e.stopPropagation();
                handleDeleteHubTerminal(terminal, worktreePath);
              }}
            >
              <Trash2 size={12} />
            </button>
          </Tooltip>
        </div>
      </div>
    );
  };

  // ==================== Render ====================

  return (
    <div className={`terminal-sessions-panel ${className}`}>
      <PanelHeader
        title={t('title')}
        actions={
          <IconButton
            size="xs"
            onClick={(e) => {
              e.stopPropagation();
              loadSessions();
            }}
            tooltip={t('actions.refresh')}
          >
            <RefreshCw size={14} />
          </IconButton>
        }
      />

      <div className="terminal-sessions-panel__content">
      <div className="terminal-sessions-panel__section">
        <div
          className="terminal-sessions-panel__section-header"
          onClick={() => setHubExpanded(!hubExpanded)}
        >
          <ChevronRight
            size={14}
            className={`terminal-sessions-panel__chevron ${hubExpanded ? 'expanded' : ''}`}
          />
          <Layers size={14} className="terminal-sessions-panel__folder-icon" />
          <span className="terminal-sessions-panel__section-title">{t('sections.terminalHub')}</span>
          
          <div className="terminal-sessions-panel__section-actions">
            <Tooltip content={t('actions.refresh')}>
              <button
                className="terminal-sessions-panel__section-action"
                onClick={(e) => {
                  e.stopPropagation();
                  handleRefreshHub();
                }}
              >
                <RefreshCw size={14} />
              </button>
            </Tooltip>
            {isGitRepo && (
              <Tooltip content={t('actions.newWorktree')}>
                <button
                  className="terminal-sessions-panel__section-action"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleAddWorktree();
                  }}
                >
                  <GitBranch size={14} />
                </button>
              </Tooltip>
            )}
            <Tooltip content={t('actions.newTerminal')}>
              <button
                className="terminal-sessions-panel__section-action"
                onClick={(e) => {
                  e.stopPropagation();
                  handleAddHubTerminal();
                }}
              >
                <Plus size={14} />
              </button>
            </Tooltip>
          </div>
        </div>

        {hubExpanded && (
          <div className="terminal-sessions-panel__section-content">
            {hubConfig.terminals.length > 0 && (
              <div className="terminal-sessions-panel__list">
                {hubConfig.terminals.map(terminal => renderTerminalItem(terminal))}
              </div>
            )}

            {worktrees.filter(wt => !wt.isMain).map((worktree) => {
              const isExpanded = expandedWorktrees.has(worktree.path);
              const terminals = hubConfig.worktrees[worktree.path] || [];
              
              return (
                <div key={worktree.path} className="terminal-sessions-panel__worktree">
                  <div
                    className="terminal-sessions-panel__worktree-header"
                    onClick={() => toggleWorktreeExpanded(worktree.path)}
                  >
                    <ChevronRight
                      size={12}
                      className={`terminal-sessions-panel__chevron ${isExpanded ? 'expanded' : ''}`}
                    />
                    <GitBranch size={14} className="terminal-sessions-panel__worktree-icon" />
                    <span className="terminal-sessions-panel__worktree-name">
                      {worktree.branch || worktree.path.split(/[/\\]/).pop()}
                    </span>
                    <span className="terminal-sessions-panel__worktree-count">
                      {terminals.length}
                    </span>
                    
                    <div className="terminal-sessions-panel__worktree-actions">
                      <Tooltip content={t('actions.newTerminal')}>
                        <button
                          className="terminal-sessions-panel__section-action"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleAddHubTerminal(worktree.path);
                          }}
                        >
                          <Plus size={12} />
                        </button>
                      </Tooltip>
                      <Tooltip content={t('actions.deleteWorktree')}>
                        <button
                          className="terminal-sessions-panel__section-action terminal-sessions-panel__section-action--danger"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleRemoveWorktree(worktree.path);
                          }}
                        >
                          <Trash2 size={12} />
                        </button>
                      </Tooltip>
                    </div>
                  </div>
                  
                  {isExpanded && (
                    <div className="terminal-sessions-panel__worktree-content">
                      {terminals.length > 0 && (
                        <div className="terminal-sessions-panel__list">
                          {terminals.map(terminal => renderTerminalItem(terminal, worktree.path))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="terminal-sessions-panel__section">
        <div
          className="terminal-sessions-panel__section-header"
          onClick={() => setTerminalsExpanded(!terminalsExpanded)}
        >
          <ChevronRight
            size={14}
            className={`terminal-sessions-panel__chevron ${terminalsExpanded ? 'expanded' : ''}`}
          />
          <Monitor size={14} className="terminal-sessions-panel__folder-icon" />
          <span className="terminal-sessions-panel__section-title">{t('sections.terminals')}</span>
          <Tooltip content={t('actions.newTerminal')}>
            <button
              className="terminal-sessions-panel__section-action"
              onClick={(e) => {
                e.stopPropagation();
                handleCreateSession();
              }}
            >
              <Plus size={14} />
            </button>
          </Tooltip>
        </div>

        {terminalsExpanded && (
          <div className="terminal-sessions-panel__section-content">
            {sessions.filter(s => !s.id.startsWith(HUB_TERMINAL_ID_PREFIX)).length > 0 && (
              <div className="terminal-sessions-panel__list">
                {sessions.filter(s => !s.id.startsWith(HUB_TERMINAL_ID_PREFIX)).map((session) => (
                  <div
                    key={session.id}
                    className="terminal-sessions-panel__item"
                    onClick={() => handleOpenSession(session)}
                  >
                    <div className="terminal-sessions-panel__item-icon">
                      <TerminalIcon size={14} />
                    </div>
                    <div className="terminal-sessions-panel__item-info">
                      <div className="terminal-sessions-panel__item-name">
                        {session.name}
                      </div>
                      <div className="terminal-sessions-panel__item-meta">
                        <span className="terminal-sessions-panel__item-shell">
                          {session.shellType}
                        </span>
                        <span className="terminal-sessions-panel__item-status">
                          {session.status === 'Running' ? (
                            <Play size={10} />
                          ) : (
                            <Square size={10} />
                          )}
                          {session.status}
                        </span>
                      </div>
                    </div>
                    <div className="terminal-sessions-panel__item-actions">
                      <Tooltip content={t('actions.closeTerminal')}>
                        <button
                          className="terminal-sessions-panel__item-action"
                          onClick={(e) => handleCloseSession(session.id, e)}
                        >
                          <Trash2 size={12} />
                        </button>
                      </Tooltip>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
      </div>

      {workspacePath && (
        <BranchSelectModal
          isOpen={branchModalOpen}
          onClose={() => setBranchModalOpen(false)}
          onSelect={handleBranchSelect}
          repositoryPath={workspacePath}
          currentBranch={currentBranch}
          existingWorktreeBranches={worktrees.map(wt => wt.branch).filter(Boolean) as string[]}
        />
      )}

      {editingTerminal && (
        <TerminalEditModal
          isOpen={editModalOpen}
          onClose={() => {
            setEditModalOpen(false);
            setEditingTerminal(null);
          }}
          onSave={handleSaveTerminalEdit}
          initialName={editingTerminal.terminal.name}
          initialStartupCommand={editingTerminal.terminal.startupCommand}
        />
      )}

      {pendingDeleteWorktree && (
        <div className="terminal-sessions-panel__confirm-overlay">
          <div className="terminal-sessions-panel__confirm-dialog">
            <div className="terminal-sessions-panel__confirm-title">{t('dialog.deleteWorktree.title')}</div>
            <div className="terminal-sessions-panel__confirm-message">
              {t('dialog.deleteWorktree.message')}
              <br />
              <span className="terminal-sessions-panel__confirm-path">
                {pendingDeleteWorktree.split(/[/\\]/).pop()}
              </span>
              <br />
              <small>{t('dialog.deleteWorktree.hint')}</small>
            </div>
            <div className="terminal-sessions-panel__confirm-actions">
              <button
                className="terminal-sessions-panel__confirm-btn terminal-sessions-panel__confirm-btn--cancel"
                onClick={cancelRemoveWorktree}
              >
                {t('dialog.deleteWorktree.cancel')}
              </button>
              <button
                className="terminal-sessions-panel__confirm-btn terminal-sessions-panel__confirm-btn--confirm"
                onClick={confirmRemoveWorktree}
              >
                {t('dialog.deleteWorktree.confirm')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default TerminalSessionsPanel;

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getTerminalService } from '@/tools/terminal';
import type { TerminalService } from '@/tools/terminal';
import type { SessionResponse, TerminalEvent } from '@/tools/terminal/types/session';
import { useTerminalSceneStore } from '@/app/stores/terminalSceneStore';
import { useSceneStore } from '@/app/stores/sceneStore';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { TerminalConfig } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useShellEntries');

const TERMINAL_HUB_STORAGE_KEY = 'bitfun-terminal-hub-config';
const HUB_TERMINAL_ID_PREFIX = 'hub_';

export interface HubTerminalEntry {
  sessionId: string;
  name: string;
  startupCommand?: string;
}

export interface HubConfig {
  terminals: HubTerminalEntry[];
  worktrees: Record<string, HubTerminalEntry[]>;
}

export interface ShellEntry {
  sessionId: string;
  name: string;
  isRunning: boolean;
  isHub: boolean;
  worktreePath?: string;
  startupCommand?: string;
}

interface EditingTerminalState {
  terminal: HubTerminalEntry;
  isHub: boolean;
  worktreePath?: string;
}

export interface UseShellEntriesReturn {
  mainEntries: ShellEntry[];
  hubMainEntries: ShellEntry[];
  adHocEntries: ShellEntry[];
  getWorktreeEntries: (worktreePath: string) => ShellEntry[];
  editModalOpen: boolean;
  editingTerminal: EditingTerminalState | null;
  closeEditModal: () => void;
  refresh: () => Promise<void>;
  createAdHocTerminal: () => Promise<void>;
  createHubTerminal: (worktreePath?: string) => Promise<void>;
  promoteToHub: (entry: ShellEntry) => void;
  openTerminal: (entry: ShellEntry) => Promise<void>;
  startTerminal: (entry: ShellEntry) => Promise<boolean>;
  stopTerminal: (sessionId: string) => Promise<void>;
  deleteTerminal: (entry: ShellEntry) => Promise<void>;
  openEditModal: (entry: ShellEntry) => void;
  saveEdit: (newName: string, newStartupCommand?: string) => void;
  closeWorktreeTerminals: (worktreePath: string) => Promise<void>;
  removeWorktreeConfig: (worktreePath: string) => void;
}

function loadHubConfig(workspacePath: string): HubConfig {
  try {
    const raw = localStorage.getItem(`${TERMINAL_HUB_STORAGE_KEY}:${workspacePath}`);
    if (raw) {
      return JSON.parse(raw) as HubConfig;
    }
  } catch (error) {
    log.error('Failed to load hub config', error);
  }

  return { terminals: [], worktrees: {} };
}

function saveHubConfig(workspacePath: string, config: HubConfig) {
  try {
    localStorage.setItem(`${TERMINAL_HUB_STORAGE_KEY}:${workspacePath}`, JSON.stringify(config));
  } catch (error) {
    log.error('Failed to save hub config', error);
  }
}

function generateHubTerminalId(): string {
  return `${HUB_TERMINAL_ID_PREFIX}${Date.now()}_${Math.random().toString(36).slice(2, 11)}`;
}

async function getDefaultShellType(): Promise<string | undefined> {
  try {
    const config = await configManager.getConfig<TerminalConfig>('terminal');
    return config?.default_shell || undefined;
  } catch {
    return undefined;
  }
}

function dispatchTerminalDestroyed(sessionId: string) {
  if (typeof window === 'undefined') {
    return;
  }

  window.dispatchEvent(new CustomEvent('terminal-session-destroyed', { detail: { sessionId } }));
}

function dispatchTerminalRenamed(sessionId: string, newName: string) {
  if (typeof window === 'undefined') {
    return;
  }

  window.dispatchEvent(new CustomEvent('terminal-session-renamed', { detail: { sessionId, newName } }));
}

export function useShellEntries(): UseShellEntriesReturn {
  const setActiveSession = useTerminalSceneStore((s) => s.setActiveSession);
  const { workspacePath } = useCurrentWorkspace();

  const [sessions, setSessions] = useState<SessionResponse[]>([]);
  const [hubConfig, setHubConfig] = useState<HubConfig>({ terminals: [], worktrees: {} });
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editingTerminal, setEditingTerminal] = useState<EditingTerminalState | null>(null);

  const serviceRef = useRef<TerminalService | null>(null);

  const runningIds = useMemo(() => new Set(sessions.map((session) => session.id)), [sessions]);

  const configuredIds = useMemo(() => {
    const ids = new Set<string>();

    hubConfig.terminals.forEach((terminal) => ids.add(terminal.sessionId));
    Object.values(hubConfig.worktrees).forEach((terminals) => {
      terminals.forEach((terminal) => ids.add(terminal.sessionId));
    });

    return ids;
  }, [hubConfig]);

  const refreshSessions = useCallback(async () => {
    const service = serviceRef.current;
    if (!service) {
      return;
    }

    try {
      setSessions(await service.listSessions());
    } catch (error) {
      log.error('Failed to list sessions', error);
    }
  }, []);

  useEffect(() => {
    if (!workspacePath) {
      setHubConfig({ terminals: [], worktrees: {} });
      return;
    }

    setHubConfig(loadHubConfig(workspacePath));
  }, [workspacePath]);

  useEffect(() => {
    const service = getTerminalService();
    serviceRef.current = service;

    const init = async () => {
      try {
        await service.connect();
        await refreshSessions();
      } catch (error) {
        log.error('Failed to connect terminal service', error);
      }
    };

    void init();

    const unsubscribe = service.onEvent((event: TerminalEvent) => {
      if (event.type === 'ready' || event.type === 'exit') {
        void refreshSessions();
      }
    });

    return () => unsubscribe();
  }, [refreshSessions]);

  const mainEntries = useMemo<ShellEntry[]>(() => {
    const hubEntries = hubConfig.terminals.map((terminal) => ({
      sessionId: terminal.sessionId,
      name: terminal.name,
      isRunning: runningIds.has(terminal.sessionId),
      isHub: true,
      startupCommand: terminal.startupCommand,
    }));

    const adHocEntries = sessions
      .filter((session) => !configuredIds.has(session.id))
      .map((session) => ({
        sessionId: session.id,
        name: session.name,
        isRunning: true,
        isHub: false,
      }));

    return [...hubEntries, ...adHocEntries];
  }, [configuredIds, hubConfig.terminals, runningIds, sessions]);

  const hubMainEntries = useMemo(
    () => mainEntries.filter((entry) => entry.isHub),
    [mainEntries],
  );

  const adHocEntries = useMemo(
    () => mainEntries.filter((entry) => !entry.isHub),
    [mainEntries],
  );

  const worktreeEntries = useMemo<Record<string, ShellEntry[]>>(() => {
    const result: Record<string, ShellEntry[]> = {};

    Object.entries(hubConfig.worktrees).forEach(([worktreePath, terminals]) => {
      result[worktreePath] = terminals.map((terminal) => ({
        sessionId: terminal.sessionId,
        name: terminal.name,
        isRunning: runningIds.has(terminal.sessionId),
        isHub: true,
        worktreePath,
        startupCommand: terminal.startupCommand,
      }));
    });

    return result;
  }, [hubConfig.worktrees, runningIds]);

  const updateHubConfig = useCallback((updater: (prev: HubConfig) => HubConfig) => {
    if (!workspacePath) {
      return;
    }

    setHubConfig((prev) => {
      const next = updater(prev);
      saveHubConfig(workspacePath, next);
      return next;
    });
  }, [workspacePath]);

  const getWorktreeEntries = useCallback(
    (worktreePath: string) => worktreeEntries[worktreePath] ?? [],
    [worktreeEntries],
  );

  const refresh = useCallback(async () => {
    await refreshSessions();

    if (workspacePath) {
      setHubConfig(loadHubConfig(workspacePath));
    }
  }, [refreshSessions, workspacePath]);

  const openInShellScene = useCallback((sessionId: string) => {
    const { openScene, openTabs, activeTabId } = useSceneStore.getState();
    // #region agent log
    console.error('[DBG-366fda][H-C] openInShellScene called', {sessionId, activeTabId, openTabIds: openTabs.map(t=>t.id)});
    // #endregion
    openScene('shell' as SceneTabId);
    const afterState = useSceneStore.getState();
    // #region agent log
    console.error('[DBG-366fda][H-C] after openScene(shell)', {newActiveTabId: afterState.activeTabId, openTabIds: afterState.openTabs.map(t=>t.id)});
    // #endregion
    setActiveSession(sessionId);
  }, [setActiveSession]);


  const startTerminal = useCallback(async (entry: ShellEntry): Promise<boolean> => {
    const service = serviceRef.current;
    // #region agent log
    console.error('[DBG-366fda][H-B] startTerminal called', {entrySessionId:entry.sessionId,entryName:entry.name,isHub:entry.isHub,isRunning:entry.isRunning,startupCommand:entry.startupCommand,workspacePath,hasService:!!service});
    // #endregion
    if (!service) {
      return false;
    }

    try {
      const shellType = await getDefaultShellType();

      const createdSession = await service.createSession({
        sessionId: entry.sessionId,
        workingDirectory: entry.worktreePath ?? workspacePath,
        name: entry.name,
        shellType,
      });
      // #region agent log
      console.error('[DBG-366fda][H-B] session created', {createdId:createdSession.id,createdStatus:createdSession.status,requestedId:entry.sessionId,idMatch:createdSession.id===entry.sessionId});
      // #endregion

      if (entry.startupCommand?.trim()) {
        await new Promise((resolve) => setTimeout(resolve, 800));
        try {
          await service.sendCommand(entry.sessionId, entry.startupCommand);
        } catch (error) {
          log.error('Failed to run startup command', error);
        }
      }

      await refreshSessions();
      return true;
    } catch (error) {
      // #region agent log
      console.error('[DBG-366fda][H-B] startTerminal FAILED', {error:String(error),entrySessionId:entry.sessionId});
      // #endregion
      log.error('Failed to start terminal', error);
      return false;
    }
  }, [refreshSessions, workspacePath]);

  const openTerminal = useCallback(async (entry: ShellEntry) => {
    // #region agent log
    console.error('[DBG-366fda][H-A] openTerminal called', {entrySessionId:entry.sessionId,isHub:entry.isHub,isRunning:entry.isRunning,startupCommand:entry.startupCommand});
    // #endregion
    if (!entry.isRunning && entry.isHub) {
      const started = await startTerminal(entry);
      // #region agent log
      console.error('[DBG-366fda][H-A] startTerminal result', {started,entrySessionId:entry.sessionId});
      // #endregion
      if (!started) {
        return;
      }
    }

    openInShellScene(entry.sessionId);
  }, [openInShellScene, startTerminal]);

  const createAdHocTerminal = useCallback(async () => {
    const service = serviceRef.current;
    if (!service) {
      return;
    }

    try {
      const shellType = await getDefaultShellType();
      const nextIndex = adHocEntries.length + 1;
      const session = await service.createSession({
        workingDirectory: workspacePath,
        name: `Shell ${nextIndex}`,
        shellType,
      });

      setSessions((prev) => [...prev, session]);
      openInShellScene(session.id);
    } catch (error) {
      log.error('Failed to create ad-hoc terminal', error);
    }
  }, [adHocEntries.length, openInShellScene, workspacePath]);

  const createHubTerminal = useCallback(async (worktreePath?: string) => {
    const service = serviceRef.current;
    if (!workspacePath || !service) {
      return;
    }

    const newEntry: HubTerminalEntry = {
      sessionId: generateHubTerminalId(),
      name: `Terminal ${Date.now() % 1000}`,
    };

    updateHubConfig((prev) => {
      if (worktreePath) {
        const terminals = prev.worktrees[worktreePath] || [];
        return {
          ...prev,
          worktrees: {
            ...prev.worktrees,
            [worktreePath]: [...terminals, newEntry],
          },
        };
      }

      return {
        ...prev,
        terminals: [...prev.terminals, newEntry],
      };
    });

    try {
      const shellType = await getDefaultShellType();
      await service.createSession({
        sessionId: newEntry.sessionId,
        workingDirectory: worktreePath ?? workspacePath,
        name: newEntry.name,
        shellType,
      });

      await refreshSessions();
      openInShellScene(newEntry.sessionId);
    } catch (error) {
      log.error('Failed to create hub terminal', error);
    }
  }, [openInShellScene, refreshSessions, updateHubConfig, workspacePath]);

  const promoteToHub = useCallback((entry: ShellEntry) => {
    if (!workspacePath || entry.isHub) {
      return;
    }

    updateHubConfig((prev) => ({
      ...prev,
      terminals: [
        ...prev.terminals,
        {
          sessionId: entry.sessionId,
          name: entry.name,
        },
      ],
    }));
  }, [updateHubConfig, workspacePath]);

  const stopTerminal = useCallback(async (sessionId: string) => {
    const service = serviceRef.current;
    if (!service || !runningIds.has(sessionId)) {
      return;
    }

    try {
      await service.closeSession(sessionId);
      dispatchTerminalDestroyed(sessionId);
      await refreshSessions();
    } catch (error) {
      log.error('Failed to stop terminal', error);
    }
  }, [refreshSessions, runningIds]);

  const deleteTerminal = useCallback(async (entry: ShellEntry) => {
    const service = serviceRef.current;

    if (entry.isRunning && service) {
      try {
        await service.closeSession(entry.sessionId);
      } catch (error) {
        log.error('Failed to close terminal session', error);
      }
    }

    dispatchTerminalDestroyed(entry.sessionId);

    if (entry.isHub) {
      updateHubConfig((prev) => {
        if (entry.worktreePath) {
          const terminals = (prev.worktrees[entry.worktreePath] || []).filter(
            (terminal) => terminal.sessionId !== entry.sessionId,
          );

          return {
            ...prev,
            worktrees: {
              ...prev.worktrees,
              [entry.worktreePath]: terminals,
            },
          };
        }

        return {
          ...prev,
          terminals: prev.terminals.filter((terminal) => terminal.sessionId !== entry.sessionId),
        };
      });
    }

    await refreshSessions();
  }, [refreshSessions, updateHubConfig]);

  const openEditModal = useCallback((entry: ShellEntry) => {
    setEditingTerminal({
      terminal: {
        sessionId: entry.sessionId,
        name: entry.name,
        startupCommand: entry.startupCommand,
      },
      isHub: entry.isHub,
      worktreePath: entry.worktreePath,
    });
    setEditModalOpen(true);
  }, []);

  const closeEditModal = useCallback(() => {
    setEditModalOpen(false);
    setEditingTerminal(null);
  }, []);

  const saveEdit = useCallback((newName: string, newStartupCommand?: string) => {
    if (!editingTerminal) {
      return;
    }

    const { terminal, worktreePath, isHub } = editingTerminal;

    if (isHub) {
      updateHubConfig((prev) => {
        if (worktreePath) {
          return {
            ...prev,
            worktrees: {
              ...prev.worktrees,
              [worktreePath]: (prev.worktrees[worktreePath] || []).map((item) =>
                item.sessionId === terminal.sessionId
                  ? { ...item, name: newName, startupCommand: newStartupCommand }
                  : item,
              ),
            },
          };
        }

        return {
          ...prev,
          terminals: prev.terminals.map((item) =>
            item.sessionId === terminal.sessionId
              ? { ...item, name: newName, startupCommand: newStartupCommand }
              : item,
          ),
        };
      });
    }

    if (runningIds.has(terminal.sessionId)) {
      setSessions((prev) =>
        prev.map((session) =>
          session.id === terminal.sessionId ? { ...session, name: newName } : session,
        ),
      );
      dispatchTerminalRenamed(terminal.sessionId, newName);
    }

    closeEditModal();
  }, [closeEditModal, editingTerminal, runningIds, updateHubConfig]);

  const closeWorktreeTerminals = useCallback(async (worktreePath: string) => {
    const service = serviceRef.current;
    if (!service) {
      return;
    }

    const terminals = hubConfig.worktrees[worktreePath] || [];
    for (const terminal of terminals) {
      if (!runningIds.has(terminal.sessionId)) {
        continue;
      }

      try {
        await service.closeSession(terminal.sessionId);
        dispatchTerminalDestroyed(terminal.sessionId);
      } catch (error) {
        log.error('Failed to close worktree terminal', error);
      }
    }

    await refreshSessions();
  }, [hubConfig.worktrees, refreshSessions, runningIds]);

  const removeWorktreeConfig = useCallback((worktreePath: string) => {
    updateHubConfig((prev) => {
      const nextWorktrees = { ...prev.worktrees };
      delete nextWorktrees[worktreePath];
      return {
        ...prev,
        worktrees: nextWorktrees,
      };
    });
  }, [updateHubConfig]);

  return {
    mainEntries,
    hubMainEntries,
    adHocEntries,
    getWorktreeEntries,
    editModalOpen,
    editingTerminal,
    closeEditModal,
    refresh,
    createAdHocTerminal,
    createHubTerminal,
    promoteToHub,
    openTerminal,
    startTerminal,
    stopTerminal,
    deleteTerminal,
    openEditModal,
    saveEdit,
    closeWorktreeTerminals,
    removeWorktreeConfig,
  };
}

import { useCallback, useEffect, useMemo, useState } from 'react';
import { gitAPI, type GitWorktreeInfo } from '@/infrastructure/api/service-api/GitAPI';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useWorktrees');

export interface UseWorktreesReturn {
  workspacePath?: string;
  worktrees: GitWorktreeInfo[];
  nonMainWorktrees: GitWorktreeInfo[];
  isGitRepo: boolean;
  currentBranch?: string;
  refresh: () => Promise<void>;
  addWorktree: (branch: string, isNew: boolean) => Promise<void>;
  removeWorktree: (worktreePath: string) => Promise<boolean>;
}

export function useWorktrees(): UseWorktreesReturn {
  const { workspacePath } = useCurrentWorkspace();

  const [worktrees, setWorktrees] = useState<GitWorktreeInfo[]>([]);
  const [isGitRepo, setIsGitRepo] = useState(false);
  const [currentBranch, setCurrentBranch] = useState<string | undefined>();

  const refresh = useCallback(async () => {
    if (!workspacePath) {
      setWorktrees([]);
      setIsGitRepo(false);
      setCurrentBranch(undefined);
      return;
    }

    try {
      const repository = await gitAPI.isGitRepository(workspacePath);
      setIsGitRepo(repository);

      if (!repository) {
        setWorktrees([]);
        setCurrentBranch(undefined);
        return;
      }

      const [worktreeList, branches] = await Promise.all([
        gitAPI.listWorktrees(workspacePath),
        gitAPI.getBranches(workspacePath, false),
      ]);

      setWorktrees(worktreeList);
      setCurrentBranch(branches.find((branch) => branch.current)?.name);
    } catch (error) {
      log.error('Failed to load worktrees', error);
      setWorktrees([]);
      setIsGitRepo(false);
      setCurrentBranch(undefined);
    }
  }, [workspacePath]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const addWorktree = useCallback(async (branch: string, isNew: boolean) => {
    if (!workspacePath) {
      return;
    }

    try {
      await gitAPI.addWorktree(workspacePath, branch, isNew);
      await refresh();
    } catch (error) {
      log.error('Failed to add worktree', error);
      throw error;
    }
  }, [refresh, workspacePath]);

  const removeWorktree = useCallback(async (worktreePath: string): Promise<boolean> => {
    if (!workspacePath) {
      return false;
    }

    try {
      await gitAPI.removeWorktree(workspacePath, worktreePath);
      await refresh();
      return true;
    } catch (error) {
      log.error('Failed to remove worktree', error);
      return false;
    }
  }, [refresh, workspacePath]);

  const nonMainWorktrees = useMemo(
    () => worktrees.filter((worktree) => !worktree.isMain),
    [worktrees],
  );

  return {
    workspacePath,
    worktrees,
    nonMainWorktrees,
    isGitRepo,
    currentBranch,
    refresh,
    addWorktree,
    removeWorktree,
  };
}

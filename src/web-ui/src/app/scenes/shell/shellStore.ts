import { create } from 'zustand';
import type { ShellNavView } from './shellConfig';
import { DEFAULT_SHELL_NAV_VIEW } from './shellConfig';

interface ShellState {
  navView: ShellNavView;
  setNavView: (view: ShellNavView) => void;
  expandedWorktrees: Set<string>;
  toggleWorktree: (path: string) => void;
}

export const useShellStore = create<ShellState>((set) => ({
  navView: DEFAULT_SHELL_NAV_VIEW,
  setNavView: (view) => set({ navView: view }),
  expandedWorktrees: new Set<string>(),
  toggleWorktree: (path) =>
    set((state) => {
      const next = new Set(state.expandedWorktrees);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return { expandedWorktrees: next };
    }),
}));

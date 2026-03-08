import { create } from 'zustand';
import type { MyAgentView } from './myAgentConfig';
import { DEFAULT_MY_AGENT_VIEW } from './myAgentConfig';

interface MyAgentState {
  activeView: MyAgentView;
  setActiveView: (view: MyAgentView) => void;
}

export const useMyAgentStore = create<MyAgentState>((set) => ({
  activeView: DEFAULT_MY_AGENT_VIEW,
  setActiveView: (view) => set({ activeView: view }),
}));

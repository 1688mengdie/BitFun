import { create } from 'zustand';
import type { MyAgentView } from './myAgentConfig';
import { DEFAULT_MY_AGENT_VIEW } from './myAgentConfig';

interface MyAgentState {
  activeView: MyAgentView;
  selectedAssistantWorkspaceId: string | null;
  setActiveView: (view: MyAgentView) => void;
  setSelectedAssistantWorkspaceId: (workspaceId: string | null) => void;
}

export const useMyAgentStore = create<MyAgentState>((set) => ({
  activeView: DEFAULT_MY_AGENT_VIEW,
  selectedAssistantWorkspaceId: null,
  setActiveView: (view) => set({ activeView: view }),
  setSelectedAssistantWorkspaceId: (workspaceId) => set({ selectedAssistantWorkspaceId: workspaceId }),
}));

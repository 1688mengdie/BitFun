import { create } from 'zustand';
import type { SessionInfo, ChatMessage } from './RemoteSessionManager';
import type { ConnectionState } from './RelayConnection';

interface MobileStore {
  connectionState: ConnectionState;
  setConnectionState: (s: ConnectionState) => void;

  sessions: SessionInfo[];
  setSessions: (s: SessionInfo[]) => void;

  activeSessionId: string | null;
  setActiveSessionId: (id: string | null) => void;

  // Per-session message storage
  messagesBySession: Record<string, ChatMessage[]>;
  getMessages: (sessionId: string) => ChatMessage[];
  setMessages: (sessionId: string, m: ChatMessage[]) => void;
  appendMessage: (sessionId: string, m: ChatMessage) => void;
  updateLastMessage: (sessionId: string, content: string) => void;

  error: string | null;
  setError: (e: string | null) => void;

  isStreaming: boolean;
  setIsStreaming: (v: boolean) => void;
}

export const useMobileStore = create<MobileStore>((set, get) => ({
  connectionState: 'disconnected',
  setConnectionState: (connectionState) => set({ connectionState }),

  sessions: [],
  setSessions: (sessions) => set({ sessions }),

  activeSessionId: null,
  setActiveSessionId: (activeSessionId) => set({ activeSessionId }),

  messagesBySession: {},
  getMessages: (sessionId: string) => {
    return get().messagesBySession[sessionId] || [];
  },
  setMessages: (sessionId, m) =>
    set((s) => ({
      messagesBySession: { ...s.messagesBySession, [sessionId]: m },
    })),
  appendMessage: (sessionId, m) =>
    set((s) => {
      const prev = s.messagesBySession[sessionId] || [];
      return {
        messagesBySession: { ...s.messagesBySession, [sessionId]: [...prev, m] },
      };
    }),
  updateLastMessage: (sessionId, content) =>
    set((s) => {
      const msgs = [...(s.messagesBySession[sessionId] || [])];
      if (msgs.length > 0) {
        msgs[msgs.length - 1] = { ...msgs[msgs.length - 1], content };
      }
      return {
        messagesBySession: { ...s.messagesBySession, [sessionId]: msgs },
      };
    }),

  error: null,
  setError: (error) => set({ error }),

  isStreaming: false,
  setIsStreaming: (isStreaming) => set({ isStreaming }),
}));

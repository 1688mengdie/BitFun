 

import React, { createContext, useContext, useState, useCallback, ReactNode } from 'react';

export type ChatMessage = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  metadata?: Record<string, any>;
}

interface ChatState {
  messages: ChatMessage[];
  input: string;
  isProcessing: boolean;
  error: string | null;
}

interface ChatActions {
  addMessage: (message: ChatMessage) => void;
  updateMessage: (messageId: string, updater: (message: ChatMessage) => ChatMessage) => void;
  setInput: (input: string) => void;
  setProcessing: (processing: boolean) => void;
  setError: (error: string | null) => void;
  clearChat: () => void;
  clearError: () => void;
}

interface ChatContextType {
  state: ChatState;
  actions: ChatActions;
}

const ChatContext = createContext<ChatContextType | undefined>(undefined);

export const useChat = () => {
  const context = useContext(ChatContext);
  if (!context) {
    throw new Error('useChat must be used within a ChatProvider');
  }
  return context;
};

interface ChatProviderProps {
  children: ReactNode;
}

export const ChatProvider: React.FC<ChatProviderProps> = ({ children }) => {
  const [state, setState] = useState<ChatState>({
    messages: [],
    input: '',
    isProcessing: false,
    error: null
  });

  const addMessage = useCallback((message: ChatMessage) => {
    setState(prev => ({
      ...prev,
      messages: [...prev.messages, message]
    }));
  }, []);

  const updateMessage = useCallback((messageId: string, updater: (message: ChatMessage) => ChatMessage) => {
    setState(prev => ({
      ...prev,
      messages: prev.messages.map(msg => 
        msg.id === messageId ? updater(msg) : msg
      )
    }));
  }, []);

  const setInput = useCallback((input: string) => {
    setState(prev => ({
      ...prev,
      input
    }));
  }, []);

  const setProcessing = useCallback((processing: boolean) => {
    setState(prev => ({
      ...prev,
      isProcessing: processing
    }));
  }, []);

  const setError = useCallback((error: string | null) => {
    setState(prev => ({
      ...prev,
      error
    }));
  }, []);

  const clearChat = useCallback(() => {
    setState(prev => ({
      ...prev,
      messages: [],
      input: '',
      error: null
    }));
  }, []);

  const clearError = useCallback(() => {
    setState(prev => ({
      ...prev,
      error: null
    }));
  }, []);

  const actions: ChatActions = {
    addMessage,
    updateMessage,
    setInput,
    setProcessing,
    setError,
    clearChat,
    clearError
  };

  const contextValue: ChatContextType = {
    state,
    actions
  };

  return (
    <ChatContext.Provider value={contextValue}>
      {children}
    </ChatContext.Provider>
  );
};

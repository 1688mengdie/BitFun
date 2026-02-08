 

import React, { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ViewModeContext');

export type ViewMode = 'agentic' | 'editor';

interface ViewModeContextType {
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
  toggleViewMode: () => void;
  isAgenticMode: boolean;
  isEditorMode: boolean;
}

const ViewModeContext = createContext<ViewModeContextType | undefined>(undefined);

interface ViewModeProviderProps {
  children: ReactNode;
  defaultMode?: ViewMode;
}

export const ViewModeProvider: React.FC<ViewModeProviderProps> = ({ 
  children, 
  defaultMode = 'agentic' 
}) => {
  const [viewMode, setViewModeState] = useState<ViewMode>(defaultMode);

  const setViewMode = useCallback((mode: ViewMode) => {
    setViewModeState(mode);
  }, []);

  const toggleViewMode = useCallback(() => {
    setViewModeState(prev => {
      const newMode = prev === 'agentic' ? 'editor' : 'agentic';
      log.debug('View mode toggled', { from: prev, to: newMode });
      return newMode;
    });
  }, []);

  const value: ViewModeContextType = {
    viewMode,
    setViewMode,
    toggleViewMode,
    isAgenticMode: viewMode === 'agentic',
    isEditorMode: viewMode === 'editor',
  };

  return (
    <ViewModeContext.Provider value={value}>
      {children}
    </ViewModeContext.Provider>
  );
};

export const useViewMode = (): ViewModeContextType => {
  const context = useContext(ViewModeContext);
  if (!context) {
    throw new Error('useViewMode must be used within ViewModeProvider');
  }
  return context;
};


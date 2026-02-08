 

import { useState, useCallback } from 'react';


interface AppState {
  
  sidebarCollapsed: boolean;
  rightPanelCollapsed: boolean;

  
  
  isLoading: boolean;
  error: string | null;
  
  
  toggleSidebar: () => void;
  toggleRightPanel: () => void;

  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  clearError: () => void;
}

 
export const useAppStore = () => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => {
    const saved = localStorage.getItem('sidebar-collapsed');
    return saved === 'true';
  });
  
  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(() => {
    const saved = localStorage.getItem('right-panel-collapsed');
    return saved !== null ? saved === 'true' : true;
  });
  

  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggleSidebar = useCallback(() => {
    setSidebarCollapsed(prev => {
      const newValue = !prev;
      localStorage.setItem('sidebar-collapsed', newValue.toString());
      return newValue;
    });
  }, []);

  const toggleRightPanel = useCallback(() => {
    setRightPanelCollapsed(prev => {
      const newValue = !prev;
      localStorage.setItem('right-panel-collapsed', newValue.toString());
      return newValue;
    });
  }, []);



  const setLoadingState = useCallback((loading: boolean) => {
    setIsLoading(loading);
  }, []);

  const setErrorState = useCallback((error: string | null) => {
    setError(error);
  }, []);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  return {
    
    sidebarCollapsed,
    rightPanelCollapsed,
    isLoading,
    error,
    
    
    toggleSidebar,
    toggleRightPanel,
    setLoading: setLoadingState,
    setError: setErrorState,
    clearError,
  };
};


export const useAppState = () => {
  const store = useAppStore();
  return {
    sidebarCollapsed: store.sidebarCollapsed,
    rightPanelCollapsed: store.rightPanelCollapsed,
    currentTheme: store.currentTheme,
    isLoading: store.isLoading,
    error: store.error,
  };
};

export const useAppActions = () => {
  const store = useAppStore();
  return {
    toggleSidebar: store.toggleSidebar,
    toggleRightPanel: store.toggleRightPanel,
    setTheme: store.setTheme,
    setLoading: store.setLoading,
    setError: store.setError,
    clearError: store.clearError,
  };
};

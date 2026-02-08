 

import { ContextGenerator, ContextInfo } from '../../../shared/utils';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ContextService');

export interface ContextServiceOptions {
  appName?: string;
  autoRefreshInterval?: number; 
  enableAutoRefresh?: boolean;
}

 
export class ContextService {
  private contextGenerator: ContextGenerator;
  private autoRefreshTimer: NodeJS.Timeout | null = null;
  private listeners: ((context: ContextInfo) => void)[] = [];
  private lastContext: ContextInfo | null = null;
  private isInitialized: boolean = false;
  
  private options: Required<ContextServiceOptions>;
  
  constructor(options: ContextServiceOptions = {}) {
    this.options = {
      appName: 'BitFun',
      autoRefreshInterval: 30 * 60 * 1000, 
      enableAutoRefresh: true,
      ...options
    };
    
    this.contextGenerator = new ContextGenerator(this.options.appName);
    
    
    
  }
  
   
  private async initializeContext(): Promise<void> {
    try {
      
      await this.contextGenerator.getContextInfo(true);
    } catch (error) {
      log.error('Failed to initialize context', error);
    }
  }
  
   
  private async ensureInitialized(): Promise<void> {
    if (!this.isInitialized) {
      await this.initializeContext();
      this.isInitialized = true;
      
      if (this.options.enableAutoRefresh) {
        this.startAutoRefresh();
      }
    }
  }
  
   
  async getCurrentContext(forceRefresh: boolean = false): Promise<ContextInfo> {
    try {
      await this.ensureInitialized();
      
      const context = await this.contextGenerator.getContextInfo(forceRefresh);
      
      
      if (!this.lastContext || this.isContextChanged(this.lastContext, context)) {
        this.lastContext = context;
        this.notifyListeners(context);
      }
      
      return context;
    } catch (error) {
      log.error('Failed to get context', error);
      throw error;
    }
  }
  
   
  async generateContextPrompt(forceRefresh: boolean = false): Promise<string> {
    try {
      await this.ensureInitialized();
      return await this.contextGenerator.generatePrompt(forceRefresh);
    } catch (error) {
      log.error('Failed to generate prompt', error);
      throw error;
    }
  }
  

  
   
  async refreshContext(): Promise<ContextInfo> {
    this.contextGenerator.clearCache();
    return await this.getCurrentContext(true);
  }
  
   
  addContextChangeListener(listener: (context: ContextInfo) => void): () => void {
    this.listeners.push(listener);
    
    
    return () => {
      const index = this.listeners.indexOf(listener);
      if (index > -1) {
        this.listeners.splice(index, 1);
      }
    };
  }
  
   
  startAutoRefresh(): void {
    if (this.autoRefreshTimer) {
      return;
    }
    
    this.autoRefreshTimer = setInterval(async () => {
      try {
        await this.getCurrentContext(true);
      } catch (error) {
        log.error('Auto refresh failed', error);
      }
    }, this.options.autoRefreshInterval);
  }
  
   
  stopAutoRefresh(): void {
    if (this.autoRefreshTimer) {
      clearInterval(this.autoRefreshTimer);
      this.autoRefreshTimer = null;
    }
  }
  
   
  destroy(): void {
    this.stopAutoRefresh();
    this.listeners = [];
    this.lastContext = null;
  }
  
   
  private isContextChanged(oldContext: ContextInfo, newContext: ContextInfo): boolean {
    return (
      oldContext.workingDirectory !== newContext.workingDirectory ||
      oldContext.currentDate !== newContext.currentDate ||
      oldContext.directoryStructure !== newContext.directoryStructure
    );
  }
  
   
  private notifyListeners(context: ContextInfo): void {
    this.listeners.forEach(listener => {
      try {
        listener(context);
      } catch (error) {
        log.error('Listener error', error);
      }
    });
  }
}


export const defaultContextService = new ContextService({
  appName: 'BitFun',
  autoRefreshInterval: 30 * 60 * 1000, 
  enableAutoRefresh: true
});

 
export const useContextService = () => {
  return defaultContextService;
};

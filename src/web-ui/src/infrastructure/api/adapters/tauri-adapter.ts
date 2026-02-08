 

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { ITransportAdapter } from './base';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('TauriAdapter');

export class TauriTransportAdapter implements ITransportAdapter {
  private unlistenFunctions: UnlistenFn[] = [];
  private connected: boolean = false;
  
   
  async connect(): Promise<void> {
    this.connected = true;
  }
  
   
  async request<T>(action: string, params?: any): Promise<T> {
    if (!this.connected) {
      await this.connect();
    }
    
    try {
      
      
      const result = params !== undefined 
        ? await invoke<T>(action, params)
        : await invoke<T>(action);
      
      return result;
    } catch (error) {
      log.error('Request failed', { action, error });
      throw error;
    }
  }
  
   
  listen<T>(event: string, callback: (data: T) => void): () => void {
    let unlistenFn: UnlistenFn | null = null;
    let isUnlistened = false;
    
    
    listen<T>(event, (e) => {
      if (!isUnlistened) {
        callback(e.payload);
      }
    }).then(fn => {
      if (isUnlistened) {
        
        fn();
      } else {
        unlistenFn = fn;
        this.unlistenFunctions.push(fn);
      }
    }).catch(error => {
      log.error('Failed to listen event', { event, error });
    });
    
    
    return () => {
      isUnlistened = true;
      if (unlistenFn) {
        unlistenFn();
        const index = this.unlistenFunctions.indexOf(unlistenFn);
        if (index > -1) {
          this.unlistenFunctions.splice(index, 1);
        }
      }
    };
  }
  
   
  async disconnect(): Promise<void> {
    
    this.unlistenFunctions.forEach(fn => {
      try {
        fn();
      } catch (error) {
        log.error('Error while unlistening', error);
      }
    });
    this.unlistenFunctions = [];
    this.connected = false;
  }
  
   
  isConnected(): boolean {
    return this.connected; 
  }
}



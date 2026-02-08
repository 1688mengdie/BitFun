 

import React from 'react';
import {
  IStore,
  StoreConfig,
  StateListener,
  StateUnsubscriber,
  StateSelector,
  StateMiddleware,
  PersistenceConfig
} from './types';
import { globalEventBus } from '../event-bus/EventBus';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('Store');

export class Store<TState extends Record<string, any>> implements IStore<TState> {
  private state: TState;
  private listeners = new Set<StateListener<TState>>();
  private config: StoreConfig<TState>;
  private middleware: StateMiddleware<TState>[];
  private destroyed = false;

  constructor(config: StoreConfig<TState>) {
    this.config = config;
    this.middleware = config.middleware || [];
    
    
    this.state = this.loadPersistedState() || config.initialState;
    
    
    if (config.validator && !config.validator(this.state)) {
      throw new Error('Invalid initial state');
    }

    
    if (config.devTools && typeof window !== 'undefined' && (window as any).__REDUX_DEVTOOLS_EXTENSION__) {
      
    }
  }

  getState(): TState {
    if (this.destroyed) {
      throw new Error('Store has been destroyed');
    }
    return { ...this.state };
  }

  setState(updater: Partial<TState> | ((prevState: TState) => TState)): void {
    if (this.destroyed) {
      throw new Error('Store has been destroyed');
    }

    const prevState = { ...this.state };
    let newState: TState;

    if (typeof updater === 'function') {
      newState = updater(prevState);
    } else {
      newState = { ...prevState, ...updater };
    }

    
    if (this.config.validator && !this.config.validator(newState)) {
      throw new Error('Invalid state update');
    }

    
    this.applyMiddleware('setState', updater, newState, (finalState) => {
      this.state = finalState;
      
      
      this.persistState();
      
      
      this.notifyListeners(finalState, prevState);
      
      
      globalEventBus.emit('state:change', {
        state: finalState,
        prevState,
        changedKeys: this.getChangedKeys(prevState, finalState)
      });
    });
  }

  subscribe(listener: StateListener<TState>): StateUnsubscriber {
    if (this.destroyed) {
      throw new Error('Store has been destroyed');
    }

    this.listeners.add(listener);
    
    return () => {
      this.listeners.delete(listener);
    };
  }

  select<TResult>(selector: StateSelector<TState, TResult>): TResult {
    if (this.destroyed) {
      throw new Error('Store has been destroyed');
    }

    try {
      return selector(this.state);
    } catch (error) {
      log.error('Error in state selector', error);
      throw error;
    }
  }

  destroy(): void {
    if (this.destroyed) {
      return;
    }

    this.listeners.clear();
    this.destroyed = true;
    
    
    if (this.config.persistence) {
      
    }
  }

  
  createSelector<TResult>(
    selector: StateSelector<TState, TResult>,
    equalityFn?: (a: TResult, b: TResult) => boolean
  ) {
    return () => {
      const [selectedState, setSelectedState] = React.useState(() => 
        this.select(selector)
      );

      React.useEffect(() => {
        const unsubscribe = this.subscribe((newState, prevState) => {
          const newSelectedState = selector(newState);
          const prevSelectedState = selector(prevState);
          
          const isEqual = equalityFn 
            ? equalityFn(newSelectedState, prevSelectedState)
            : Object.is(newSelectedState, prevSelectedState);
          
          if (!isEqual) {
            setSelectedState(newSelectedState);
          }
        });

        return unsubscribe;
      }, []);

      return selectedState;
    };
  }

  private notifyListeners(newState: TState, prevState: TState): void {
    this.listeners.forEach(listener => {
      try {
        listener(newState, prevState);
      } catch (error) {
        log.error('Error in state listener', error);
      }
    });
  }

  private applyMiddleware(
    action: string,
    payload: any,
    newState: TState,
    next: (state: TState) => void
  ): void {
    if (this.middleware.length === 0) {
      next(newState);
      return;
    }

    let index = 0;
    const dispatch = (state: TState) => {
      if (index >= this.middleware.length) {
        next(state);
        return;
      }

      const middleware = this.middleware[index++];
      middleware(action, payload, state, dispatch);
    };

    dispatch(newState);
  }

  private getChangedKeys(prevState: TState, newState: TState): string[] {
    const changedKeys: string[] = [];
    
    
    for (const key in newState) {
      if (newState[key] !== prevState[key]) {
        changedKeys.push(key);
      }
    }
    
    
    for (const key in prevState) {
      if (!(key in newState)) {
        changedKeys.push(key);
      }
    }
    
    return changedKeys;
  }

  private loadPersistedState(): TState | null {
    if (!this.config.persistence) {
      return null;
    }

    try {
      const { key, storage = localStorage, deserialize } = this.config.persistence;
      const data = storage.getItem(key);
      
      if (!data) {
        return null;
      }

      const state = deserialize ? deserialize(data) : JSON.parse(data);
      
      
      return this.filterPersistedState(state);
    } catch (error) {
      log.error('Failed to load persisted state', error);
      return null;
    }
  }

  private persistState(): void {
    if (!this.config.persistence) {
      return;
    }

    try {
      const { key, storage = localStorage, serialize } = this.config.persistence;
      const stateToSave = this.filterPersistedState(this.state);
      const data = serialize ? serialize(stateToSave) : JSON.stringify(stateToSave);
      
      storage.setItem(key, data);
    } catch (error) {
      log.error('Failed to persist state', error);
    }
  }

  private filterPersistedState(state: TState): TState {
    const { whitelist, blacklist } = this.config.persistence || {};
    
    if (whitelist) {
      const filtered = {} as TState;
      whitelist.forEach(key => {
        if (key in state) {
          (filtered as any)[key] = state[key];
        }
      });
      return filtered;
    }
    
    if (blacklist) {
      const filtered = { ...state };
      blacklist.forEach(key => {
        delete (filtered as any)[key];
      });
      return filtered;
    }
    
    return state;
  }
}



export function useStore<TState, TResult = TState>(
  store: Store<TState>,
  selector?: StateSelector<TState, TResult>
): TResult {
  const [state, setState] = React.useState(() => 
    selector ? store.select(selector) : store.getState() as unknown as TResult
  );

  React.useEffect(() => {
    const unsubscribe = store.subscribe((newState, prevState) => {
      if (selector) {
        const newSelectedState = selector(newState);
        const prevSelectedState = selector(prevState);
        
        if (!Object.is(newSelectedState, prevSelectedState)) {
          setState(newSelectedState);
        }
      } else {
        setState(newState as unknown as TResult);
      }
    });

    return unsubscribe;
  }, [store, selector]);

  return state;
}


export function createLoggerMiddleware<TState>(): StateMiddleware<TState> {
  return (action, payload, state, next) => {
    log.debug('State action', { action, payload });
    next(state);
  };
}

export function createPersistenceMiddleware<TState>(
  config: PersistenceConfig<TState>
): StateMiddleware<TState> {
  return (action, payload, state, next) => {
    next(state);
    
    
    setTimeout(() => {
      try {
        const { key, storage = localStorage, serialize } = config;
        const data = serialize ? serialize(state) : JSON.stringify(state);
        storage.setItem(key, data);
      } catch (error) {
        log.error('Persistence middleware error', error);
      }
    }, 0);
  };
}

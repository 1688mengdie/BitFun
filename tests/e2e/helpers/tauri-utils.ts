/**
 * Tauri-specific utilities (IPC, window, mocks).
 */
import { browser } from '@wdio/globals';

interface TauriCommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export async function invokeCommand<T = unknown>(
  command: string,
  args?: Record<string, unknown>
): Promise<TauriCommandResult<T>> {
  try {
    const result = await browser.execute(
      async (cmd: string, cmdArgs: Record<string, unknown> | undefined) => {
        try {
          // @ts-ignore - Tauri API available in runtime
          const { invoke } = await import('@tauri-apps/api/core');
          const data = await invoke(cmd, cmdArgs);
          return { success: true, data };
        } catch (error) {
          return { 
            success: false, 
            error: error instanceof Error ? error.message : String(error) 
          };
        }
      },
      command,
      args
    );
    
    return result as TauriCommandResult<T>;
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export async function getAppVersion(): Promise<string | null> {
  const result = await browser.execute(async () => {
    try {
      // @ts-ignore
      const { getVersion } = await import('@tauri-apps/api/app');
      return await getVersion();
    } catch {
      return null;
    }
  });
  return result;
}

export async function getAppName(): Promise<string | null> {
  const result = await browser.execute(async () => {
    try {
      // @ts-ignore
      const { getName } = await import('@tauri-apps/api/app');
      return await getName();
    } catch {
      return null;
    }
  });
  return result;
}

export async function isTauriAvailable(): Promise<boolean> {
  const result = await browser.execute(() => {
    // @ts-ignore
    return typeof window.__TAURI__ !== 'undefined';
  });
  return result;
}

export async function emitEvent(
  event: string,
  payload?: unknown
): Promise<boolean> {
  try {
    await browser.execute(
      async (eventName: string, eventPayload: unknown) => {
        // @ts-ignore
        const { emit } = await import('@tauri-apps/api/event');
        await emit(eventName, eventPayload);
      },
      event,
      payload
    );
    return true;
  } catch (error) {
    console.error('Failed to emit event:', error);
    return false;
  }
}

export async function getWindowInfo(): Promise<{
  label: string;
  title: string;
  isVisible: boolean;
  isMaximized: boolean;
  isMinimized: boolean;
} | null> {
  try {
    const result = await browser.execute(async () => {
      try {
        // @ts-ignore
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const win = getCurrentWindow();
        
        return {
          label: win.label,
          title: await win.title(),
          isVisible: await win.isVisible(),
          isMaximized: await win.isMaximized(),
          isMinimized: await win.isMinimized(),
        };
      } catch {
        return null;
      }
    });
    return result;
  } catch {
    return null;
  }
}

export async function minimizeWindow(): Promise<boolean> {
  try {
    await browser.execute(async () => {
      // @ts-ignore
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      await win.minimize();
    });
    return true;
  } catch {
    return false;
  }
}

export async function maximizeWindow(): Promise<boolean> {
  try {
    await browser.execute(async () => {
      // @ts-ignore
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      await win.maximize();
    });
    return true;
  } catch {
    return false;
  }
}

export async function unmaximizeWindow(): Promise<boolean> {
  try {
    await browser.execute(async () => {
      // @ts-ignore
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      await win.unmaximize();
    });
    return true;
  } catch {
    return false;
  }
}

export async function setWindowSize(width: number, height: number): Promise<boolean> {
  try {
    await browser.execute(
      async (w: number, h: number) => {
        // @ts-ignore
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        // @ts-ignore
        const { LogicalSize } = await import('@tauri-apps/api/dpi');
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(w, h));
      },
      width,
      height
    );
    return true;
  } catch {
    return false;
  }
}

export async function mockIPCResponse(
  command: string,
  response: unknown
): Promise<void> {
  await browser.execute(
    (cmd: string, res: unknown) => {
      // @ts-ignore
      window.__E2E_MOCKS__ = window.__E2E_MOCKS__ || {};
      // @ts-ignore
      window.__E2E_MOCKS__[cmd] = res;
    },
    command,
    response
  );
}

export async function clearMocks(): Promise<void> {
  await browser.execute(() => {
    // @ts-ignore
    window.__E2E_MOCKS__ = {};
  });
}

export async function getAppState<T = unknown>(storeName: string): Promise<T | null> {
  try {
    const result = await browser.execute((name: string) => {
      // @ts-ignore
      const store = window.__STORES__?.[name];
      return store ? store.getState() : null;
    }, storeName);
    return result as T;
  } catch {
    return null;
  }
}

export default {
  invokeCommand,
  getAppVersion,
  getAppName,
  isTauriAvailable,
  emitEvent,
  getWindowInfo,
  minimizeWindow,
  maximizeWindow,
  unmaximizeWindow,
  setWindowSize,
  mockIPCResponse,
  clearMocks,
  getAppState,
};

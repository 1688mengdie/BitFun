import ReactDOM from "react-dom/client";
import App from "./app/App";
import AppErrorBoundary from "./app/components/AppErrorBoundary";
import { WorkspaceProvider } from "./infrastructure/contexts/WorkspaceContext";
import "./app/styles/index.scss";

// Manually import Monaco Editor CSS.
// This ensures the CSS loads correctly in Tauri production.
import 'monaco-editor/min/vs/editor/editor.main.css';

// Font: Noto Sans SC is loaded via a <link> tag in index.html.
// File path: public/fonts/fonts.css, served as /fonts/fonts.css.

import { initializeAllTools } from "./tools";
import { initContextMenuSystem } from "./shared/context-menu-system";
import { loader } from '@monaco-editor/react';
import { getMonacoPath, getMonacoWorkerPath, logMonacoResourceCheck } from './tools/editor/utils/monacoPathHelper';
import { createLogger } from './shared/utils/logger';

const log = createLogger('App');

// Crash log deduplication flag
const CRASH_LOGGED_FLAG = '__bitfun_frontend_crash_logged__';
function hasLoggedCrash(): boolean {
  return Boolean((window as any)[CRASH_LOGGED_FLAG]);
}
function markCrashLogged(): void {
  (window as any)[CRASH_LOGGED_FLAG] = true;
}

function serializeError(err: unknown): Record<string, unknown> {
  if (err instanceof Error) {
    return {
      name: err.name,
      message: err.message,
      stack: err.stack,
    };
  }
  return { value: String(err) };
}

function isRootEmpty(): boolean {
  const root = document.getElementById('root');
  if (!root) {
    return true;
  }
  return root.childElementCount === 0;
}

function registerGlobalErrorHandlers() {
  const flag = '__bitfun_global_error_handlers_registered__';
  const w = window as any;
  if (w[flag]) {
    return;
  }
  w[flag] = true;

  const scheduleCrashLog = (payload: { location: string; message: string; data?: Record<string, unknown> }) => {
    // Only persist when it looks like a real "white screen"/startup crash.
    queueMicrotask(() => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (isRootEmpty() && !hasLoggedCrash()) {
            markCrashLogged();
            log.error('[CRASH] Application crashed', {
              location: payload.location,
              message: payload.message,
              ...payload.data,
            });
          }
        });
      });
    });
  };

  window.addEventListener(
    'error',
    (event: Event) => {
      if (event instanceof ErrorEvent) {
        scheduleCrashLog({
          location: 'window:error',
          message: event.message || 'window error',
          data: {
            filename: event.filename,
            lineno: event.lineno,
            colno: event.colno,
            error: serializeError(event.error),
          },
        });
        return;
      }

    // Resource load errors rarely cause a white screen; log only if root is empty.
      const target = event.target as any;
      scheduleCrashLog({
        location: 'window:resource-error',
        message: 'resource load error',
        data: {
          tagName: target?.tagName,
          src: target?.src,
          href: target?.href,
        },
      });
    },
    true
  );

  window.addEventListener('unhandledrejection', (event: PromiseRejectionEvent) => {
    scheduleCrashLog({
      location: 'window:unhandledrejection',
      message: 'unhandled rejection',
      data: {
        reason: serializeError(event.reason),
      },
    });
  });
}

registerGlobalErrorHandlers();

// Configure Monaco Editor loader - use local files (offline-ready).
const isDev = import.meta.env.DEV;
const monacoPath = getMonacoPath();

loader.config({
  paths: {
    vs: monacoPath
  }
});

log.debug('Monaco loader configured', { vs: monacoPath, isDev });

// Debug: check resource availability in production.
if (!isDev) {
  // Delay checks to avoid blocking startup.
  setTimeout(() => {
    logMonacoResourceCheck().catch(err => {
      log.error('Monaco resource check failed', err);
    });
  }, 2000);
}

// Optimization: Monaco Editor worker mapping.
const MONACO_WORKER_MAP: Record<string, string> = {
  json: 'language/json/jsonWorker.js',
  css: 'language/css/cssWorker.js',
  scss: 'language/css/cssWorker.js',
  less: 'language/css/cssWorker.js',
  html: 'language/html/htmlWorker.js',
  handlebars: 'language/html/htmlWorker.js',
  razor: 'language/html/htmlWorker.js',
  typescript: 'language/typescript/tsWorker.js',
  javascript: 'language/typescript/tsWorker.js',
};

const DEFAULT_WORKER = 'base/worker/workerMain.js';

(window as any).MonacoEnvironment = {
  getWorker(_workerId: string, label: string) {
    const workerFile = MONACO_WORKER_MAP[label] || DEFAULT_WORKER;
    const workerPath = getMonacoWorkerPath(workerFile);
    
    return new Worker(workerPath, {
      type: 'classic',
      name: `monaco-${label}-worker`
    });
  }
};

// Initialize app.
async function initializeApp() {
  try {
    // Initialize logger first (attaches console in dev mode)
    const { initLogger } = await import('./shared/utils/logger');
    await initLogger();
    
    log.info('Initializing BitFun');
    
    // Synchronous initialization: core systems that must run first.
    const { registerDefaultContextTypes } = await import('./shared/context-system/core/registerDefaultTypes');
    registerDefaultContextTypes();
    
    // Initialize smart recommendation system.
    const { initRecommendationProviders } = await import('./flow_chat/components/smart-recommendations');
    initRecommendationProviders();
    
    // Initialize theme system.
    const { themeService } = await import('./infrastructure/theme');
    await themeService.initialize();
    log.info('Theme system initialized');
    
    // Preload editor configuration.
    const { configManager } = await import('./infrastructure/config');
    await configManager.getConfig('editor');
    log.info('Editor configuration preloaded');
    
    // Note: i18n is initialized by I18nProvider, not here.
    // This avoids blocking startup and ensures i18n is ready during React render.
    
    // Parallel initialization: independent systems.
    const initResults = await Promise.allSettled([
      // Snapshot system - lazy load: initialize on first use.
      // SandboxInitializer.initialize(),
      
      // Feature module initialization.
      initializeAllTools(),
      
      // Context menu system initialization.
      (async () => {
        initContextMenuSystem({
          registerBuiltinCommands: true,
          registerBuiltinProviders: true,
          debug: false  // Disable debug mode to reduce log output.
        });
        
        // Register notification context menu.
        const { registerNotificationContextMenu } = await import('./shared/notification-system');
        registerNotificationContextMenu();
      })(),
      
      // Editor preload (Monaco).
      (async () => {
        const { MonacoManager } = await import('./tools/editor');
        await MonacoManager.initialize();  // Preload Monaco Editor.
        
        // Initialize Monaco theme sync.
        const { monacoThemeSync } = await import('./infrastructure/theme/integrations/MonacoThemeSync');
        await monacoThemeSync.initialize();
        log.info('Monaco theme sync initialized');
      })()
    ]);
    
    // Check initialization results.
    initResults.forEach((result, index) => {
      const names = ['Tools', 'ContextMenu', 'Editors'];
      if (result.status === 'rejected') {
        log.warn('Initialization failed', { module: names[index], error: result.reason });
      }
    });
    
    log.info('BitFun core systems initialized successfully');
  } catch (error) {
    log.error('Failed to initialize BitFun', error);
  }
}

// Start initialization.
initializeApp();

// I18n Provider.
import { I18nProvider } from './infrastructure/i18n';

// Render app (single-window mode; toolbar via window transform).
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AppErrorBoundary>
    <I18nProvider>
      <WorkspaceProvider>
        <App />
      </WorkspaceProvider>
    </I18nProvider>
  </AppErrorBoundary>
);

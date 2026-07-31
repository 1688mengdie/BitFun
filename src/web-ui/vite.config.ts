import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { VitePWA } from "vite-plugin-pwa";
import { versionInjectionPlugin } from "./vite.config.version-plugin";
import { bitfunCanvasRuntimeBundlePlugin } from "./vite.config.canvas-runtime-plugin";

const host = process.env.TAURI_DEV_HOST;

/**
 * Native fs events do not work reliably on UNC network shares (\\server\...,
 * including \\wsl$ / \\wsl.localhost) or on WSL drvfs mounts (/mnt/<drive>).
 * Users upgrading from the polling-based watcher would silently lose HMR
 * there, so print a one-line hint pointing at the VITE_USE_POLLING escape
 * hatch.
 */
function warnIfNativeWatchUnreliable(): void {
  const cwd = process.cwd();
  const looksLikeNetworkOrWslMount =
    cwd.startsWith("\\\\") || /^\/mnt\/[a-z]\//i.test(cwd);
  if (looksLikeNetworkOrWslMount) {
    console.warn(
      `[bitfun] Project path "${cwd}" looks like a network share or WSL mount; ` +
        "native file watching may miss changes here. " +
        "Set VITE_USE_POLLING=1 to restore polling-based HMR.",
    );
  }
}

// https://vite.dev/config/
export default defineConfig(({ mode, command }) => {
  const isProduction = mode === 'production' || (command === 'build' && mode !== 'development');

  if (command === 'serve' && !process.env.VITE_USE_POLLING) {
    warnIfNativeWatchUnreliable();
  }

  return {
    plugins: [
      react(),
      bitfunCanvasRuntimeBundlePlugin(),
      versionInjectionPlugin(),
      // PWA only for web mode — Tauri desktop does not need a service worker
      ...(command === 'build' && mode !== 'desktop'
        ? [VitePWA({
            registerType: 'autoUpdate',
            includeAssets: [
              'taiji-icon-128.png',
              'Logo-ICON*.png',
              'BitFun-Logo.png',
              'fonts/**/*.woff2',
            ],
            manifest: {
              name: 'BitFun - AI Code Assistant & 太极多维量化系统',
              short_name: 'BitFun',
              description: 'AI Code Assistant 与 LVPA 太极多维量化系统前端',
              theme_color: '#121214',
              background_color: '#121214',
              display: 'standalone',
              orientation: 'any',
              start_url: '/',
              scope: '/',
              categories: ['productivity', 'development', 'finance'],
              lang: 'zh-CN',
              icons: [
                {
                  src: 'taiji-icon-128.png',
                  sizes: '128x128',
                  type: 'image/png',
                },
                {
                  src: 'Logo-ICON-128.png',
                  sizes: '128x128',
                  type: 'image/png',
                  purpose: 'any maskable',
                },
                {
                  src: 'Logo-ICON.png',
                  sizes: '512x512',
                  type: 'image/png',
                  purpose: 'any maskable',
                },
              ],
            },
            workbox: {
              globPatterns: ['**/*.{js,css,html,woff2,png,svg,ico,webp}'],
              maximumFileSizeToCacheInBytes: 10 * 1024 * 1024,
              runtimeCaching: [
                // API calls: Network First with 30s timeout, fallback to cache
                {
                  urlPattern: /^https?:\/\/.*\/api\/.*/i,
                  handler: 'NetworkFirst',
                  options: {
                    networkTimeoutSeconds: 30,
                    cacheName: 'api-cache',
                    expiration: {
                      maxEntries: 128,
                      maxAgeSeconds: 60 * 60, // 1 hour
                    },
                    backgroundSync: {
                      name: 'api-sync-queue',
                      options: {
                        maxRetentionTime: 24 * 60, // 24 hours
                      },
                    },
                  },
                },
                // Static assets: Stale While Revalidate
                {
                  urlPattern: /\.(?:png|jpg|jpeg|svg|gif|ico|webp|woff2?)$/,
                  handler: 'StaleWhileRevalidate',
                  options: {
                    cacheName: 'static-assets',
                    expiration: {
                      maxEntries: 256,
                      maxAgeSeconds: 60 * 60 * 24 * 30, // 30 days
                    },
                  },
                },
                // Fonts: Cache First (immutable)
                {
                  urlPattern: /\/fonts\/.*/,
                  handler: 'CacheFirst',
                  options: {
                    cacheName: 'fonts',
                    expiration: {
                      maxEntries: 32,
                      maxAgeSeconds: 60 * 60 * 24 * 365, // 1 year
                    },
                  },
                },
              ],
            },
          })]
        : []),
    ],

    // Path resolution
    resolve: {
      dedupe: ['react', 'react-dom'],
      alias: {
        "@": path.resolve(__dirname, "./src"),
        "@/shared": path.resolve(__dirname, "./src/shared"),
        "@/core": path.resolve(__dirname, "./src/core"),
        "@/tools": path.resolve(__dirname, "./src/tools"),
        "@/hooks": path.resolve(__dirname, "./src/hooks"),
        "@/styles": path.resolve(__dirname, "./src/component-library/styles"),
        "@/types": path.resolve(__dirname, "./src/shared/types"),
        "@/utils": path.resolve(__dirname, "./src/shared/utils"),
        "@components": path.resolve(__dirname, "./src/component-library/components"),
      },
    },

  css: {
    preprocessorOptions: {
      scss: {
        // SCSS preprocessing options (sourcemap is controlled by build.sourcemap)
      },
    },
    // dev mode enabled, release mode disabled
    devSourcemap: !isProduction,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1422,
    // Tauri devUrl is fixed to http://localhost:1422.
    // If Vite silently falls back to another port, the desktop webview stays blank.
    strictPort: true,
    host: host || "localhost",
    hmr: {
      protocol: "ws",
      host: host || "localhost",
      port: 1421,
    },
    // Allow access to workspace root for dependencies like monaco-editor
    fs: {
      allow: [
        path.resolve(__dirname, '../../'), // Workspace root
      ],
    },
    watch: {
      // 3. tell Vite to ignore watching `src-tauri` and `apps`
      ignored: ["**/src-tauri/**", "**/apps/**"],
      // Native fs events by default (polling burned CPU scanning ~1.7k files
      // every 100ms). Escape hatch for network drives / exotic filesystems:
      // set VITE_USE_POLLING=1 to re-enable polling.
      ...(process.env.VITE_USE_POLLING
        ? { usePolling: true, interval: 1000 }
        : {}),
    },
  },

  // Optimize dependency pre-building
  optimizeDeps: {
    // Exclude dependencies that need to be dynamically loaded
    exclude: [],
    // Force pre-building dependencies
    // Resolve Vite 7 and React 18 compatibility issues
    include: [
      'react',
      'react-dom',
      'react-dom/client',
      'react/jsx-runtime',
      'react/jsx-dev-runtime',
      'mermaid',
      'mermaid/dist/mermaid.esm.min.mjs',
    ],
  },

  // Build options
  build: {
    // Enable CSS code splitting
    cssCodeSplit: true,
    // release version disable sourcemap, dev/debug version enable
    sourcemap: !isProduction,
    // Output to the project root directory dist/
    outDir: '../../dist',
    // Empty the output directory
    emptyOutDir: true,
  }
  };
});

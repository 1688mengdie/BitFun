/**
 * BeeColonyMonitor — fixed floating panel that renders the bee-colony-dag
 * MiniApp DAG visualization. Always accessible via a nav button; stays
 * visible alongside other content without taking a full scene tab.
 *
 * Pattern: FloatingMiniChat-style floating panel with MiniAppRunner inside.
 */
import React, { useState, useCallback, useEffect, useMemo } from 'react';
import { GitBranch, X, Minimize2, Maximize2 } from 'lucide-react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useAppearance } from '@/infrastructure/appearance/hooks/useAppearance';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import MiniAppRunner from '@/app/scenes/miniapps/components/MiniAppRunner';
import { useSceneStore } from '@/app/stores/sceneStore';
import './BeeColonyMonitor.scss';

const log = createLogger('BeeColonyMonitor');

const BEE_COLONY_APP_ID = 'bee-colony-dag';

export const BeeColonyMonitor: React.FC = () => {
  const { current } = useAppearance();
  const themeType = current?.mode;
  const { workspacePath } = useCurrentWorkspace();
  const activeTabId = useSceneStore((s) => s.activeTabId);

  const [isOpen, setIsOpen] = useState(false);
  const [app, setApp] = useState<MiniApp | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [maximized, setMaximized] = useState(false);

  // Only show in agent scene (where the DAG is relevant)
  const isAgentScene = useMemo(
    () => typeof activeTabId === 'string' && activeTabId.startsWith('agentic:'),
    [activeTabId],
  );

  const loadApp = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const loaded = await miniAppAPI.getMiniApp(
        BEE_COLONY_APP_ID,
        themeType ?? 'dark',
        workspacePath || undefined,
      );
      if (!loaded?.compiled_html?.trim()) {
        setError('MiniApp not compiled');
        setApp(null);
        return;
      }
      setApp(loaded);
    } catch (err) {
      log.error('Failed to load bee colony MiniApp', err);
      setError(String(err));
      setApp(null);
    } finally {
      setLoading(false);
    }
  }, [themeType, workspacePath]);

  // Load app when panel opens
  useEffect(() => {
    if (isOpen && !app && !loading) {
      void loadApp();
    }
  }, [isOpen, app, loading, loadApp]);

  const handleToggle = useCallback(() => {
    setIsOpen((prev) => !prev);
  }, []);

  const handleClose = useCallback(() => {
    setIsOpen(false);
  }, []);

  // Don't render in non-agent scenes
  if (!isAgentScene) return null;

  return (
    <div
      className={['bee-monitor', isOpen && 'bee-monitor--open'].filter(Boolean).join(' ')}
      data-bf-component="bee-colony-monitor"
      data-bf-part="root"
    >
      {/* Backdrop */}
      {isOpen && (
        <div
          className="bee-monitor__backdrop"
          onClick={handleClose}
          data-bf-component="bee-colony-monitor"
          data-bf-part="backdrop"
        />
      )}

      {/* Trigger button — always visible in agent scenes */}
      <button
        type="button"
        className="bee-monitor__button"
        onClick={handleToggle}
        title="Bee colony architecture monitor"
        aria-label="Bee colony architecture monitor"
        data-bf-component="bee-colony-monitor"
        data-bf-part="trigger"
      >
        <GitBranch size={18} />
      </button>

      {/* Floating panel */}
      <div
        className={[
          'bee-monitor__panel',
          isOpen && 'bee-monitor__panel--open',
          maximized && 'bee-monitor__panel--maximized',
        ].filter(Boolean).join(' ')}
        data-bf-component="bee-colony-monitor"
        data-bf-part="panel"
      >
        {/* Header */}
        <div
          className="bee-monitor__header"
          data-bf-component="bee-colony-monitor"
          data-bf-part="header"
        >
          <span className="bee-monitor__title">Bee colony architecture monitor</span>
          <div className="bee-monitor__header-actions">
            <button
              type="button"
              className="bee-monitor__header-btn"
              onClick={() => setMaximized((v) => !v)}
              title={maximized ? 'Restore' : 'Maximize'}
            >
              {maximized ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
            </button>
            <button
              type="button"
              className="bee-monitor__header-btn bee-monitor__header-btn--close"
              onClick={handleClose}
              title="Close"
            >
              <X size={14} />
            </button>
          </div>
        </div>

        {/* Body */}
        <div
          className="bee-monitor__body"
          data-bf-component="bee-colony-monitor"
          data-bf-part="body"
        >
          {loading && (
            <div className="bee-monitor__loading">Loading...</div>
          )}
          {error && !app && (
            <div className="bee-monitor__error">
              <p>Bee colony MiniApp not ready</p>
              <small>{error}. Make sure it is compiled and deployed.</small>
            </div>
          )}
          {app && <MiniAppRunner key={app.id} app={app} />}
        </div>
      </div>
    </div>
  );
};

export default BeeColonyMonitor;

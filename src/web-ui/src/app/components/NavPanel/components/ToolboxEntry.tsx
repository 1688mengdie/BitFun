import React, { useEffect, useMemo, useCallback } from 'react';
import { Boxes } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useToolboxStore } from '@/app/scenes/toolbox/toolboxStore';
import { renderMiniAppIcon, getMiniAppIconGradient } from '@/app/scenes/toolbox/utils/miniAppIcons';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ToolboxEntry');
const MAX_VISIBLE_RUNNING_APPS = 3;

interface ToolboxEntryProps {
  isActive: boolean;
  activeMiniAppId?: string | null;
  onOpenToolbox: () => void;
  onOpenMiniApp: (appId: string) => void;
}

const ToolboxEntry: React.FC<ToolboxEntryProps> = ({
  isActive,
  activeMiniAppId = null,
  onOpenToolbox,
  onOpenMiniApp,
}) => {
  const { t } = useI18n('common');
  const apps = useToolboxStore((s) => s.apps);
  const setApps = useToolboxStore((s) => s.setApps);
  const runningWorkerIds = useToolboxStore((s) => s.runningWorkerIds);
  const setRunningWorkerIds = useToolboxStore((s) => s.setRunningWorkerIds);
  const markWorkerRunning = useToolboxStore((s) => s.markWorkerRunning);
  const markWorkerStopped = useToolboxStore((s) => s.markWorkerStopped);

  const refreshApps = useCallback(async () => {
    try {
      const nextApps = await miniAppAPI.listMiniApps();
      setApps(nextApps);
    } catch (error) {
      log.error('Failed to refresh miniapps for toolbox entry', error);
    }
  }, [setApps]);

  useEffect(() => {
    void refreshApps();
    miniAppAPI.workerListRunning().then(setRunningWorkerIds).catch(() => {});

    const unlistenCreated = api.listen('miniapp-created', () => {
      void refreshApps();
    });
    const unlistenUpdated = api.listen('miniapp-updated', () => {
      void refreshApps();
    });
    const unlistenDeleted = api.listen<{ id?: string }>('miniapp-deleted', (payload) => {
      if (payload?.id) {
        markWorkerStopped(payload.id);
      }
      void refreshApps();
    });
    const unlistenRestarted = api.listen<{ id?: string }>('miniapp-worker-restarted', (payload) => {
      if (payload?.id) {
        markWorkerRunning(payload.id);
      }
    });
    const unlistenStopped = api.listen<{ id?: string }>('miniapp-worker-stopped', (payload) => {
      if (payload?.id) {
        markWorkerStopped(payload.id);
      }
    });

    return () => {
      unlistenCreated();
      unlistenUpdated();
      unlistenDeleted();
      unlistenRestarted();
      unlistenStopped();
    };
  }, [markWorkerRunning, markWorkerStopped, refreshApps, setRunningWorkerIds]);

  const runningApps = useMemo(() => {
    const appMap = new Map(apps.map((app) => [app.id, app]));
    const list = runningWorkerIds
      .map((id) => appMap.get(id))
      .filter((app): app is NonNullable<typeof app> => !!app);

    if (!activeMiniAppId) {
      return list;
    }

    return [...list].sort((a, b) => {
      if (a.id === activeMiniAppId) return -1;
      if (b.id === activeMiniAppId) return 1;
      return 0;
    });
  }, [activeMiniAppId, apps, runningWorkerIds]);

  const visibleApps = runningApps.slice(0, MAX_VISIBLE_RUNNING_APPS);
  const overflowCount = Math.max(0, runningApps.length - visibleApps.length);

  return (
    <div className="bitfun-nav-panel__toolbox-entry-wrap">
      <div
        className={[
          'bitfun-nav-panel__toolbox-entry',
          isActive && 'is-active',
          runningApps.length > 0 && 'has-running-apps',
        ].filter(Boolean).join(' ')}
        onClick={onOpenToolbox}
        onKeyDown={(e) => {
          if (e.currentTarget !== e.target) return;
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onOpenToolbox();
          }
        }}
        role="button"
        tabIndex={0}
        aria-label={t('scenes.toolbox')}
      >
        <span className="bitfun-nav-panel__toolbox-entry-main">
          <span className="bitfun-nav-panel__toolbox-entry-icon" aria-hidden="true">
            <Boxes size={18} />
          </span>
          <span className="bitfun-nav-panel__toolbox-entry-copy">
            <span className="bitfun-nav-panel__toolbox-entry-title">{t('scenes.toolbox')}</span>
          </span>
        </span>

        <span className="bitfun-nav-panel__toolbox-entry-apps">
          {visibleApps.length > 0 ? (
            <>
              {visibleApps.map((app) => {
                const isAppActive = app.id === activeMiniAppId;
                return (
                  <Tooltip key={app.id} content={app.name} placement="right">
                    <span
                      className={[
                        'bitfun-nav-panel__toolbox-app-bubble',
                        isAppActive && 'is-active',
                      ].filter(Boolean).join(' ')}
                      style={{ background: getMiniAppIconGradient(app.icon || 'box') }}
                      onClick={(e) => {
                        e.stopPropagation();
                        onOpenMiniApp(app.id);
                      }}
                      onMouseDown={(e) => e.stopPropagation()}
                      role="button"
                      tabIndex={0}
                      aria-label={app.name}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          e.stopPropagation();
                          onOpenMiniApp(app.id);
                        }
                      }}
                    >
                      {renderMiniAppIcon(app.icon || 'box', 14)}
                    </span>
                  </Tooltip>
                );
              })}
              {overflowCount > 0 ? (
                <span className="bitfun-nav-panel__toolbox-app-bubble bitfun-nav-panel__toolbox-app-bubble--more">
                  +{overflowCount}
                </span>
              ) : null}
            </>
          ) : null}
        </span>
      </div>
    </div>
  );
};

export default ToolboxEntry;

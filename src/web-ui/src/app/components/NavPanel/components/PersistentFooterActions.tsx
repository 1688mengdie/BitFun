import React, { useState, useCallback } from 'react';
import { Settings, Info, MoreVertical, PictureInPicture2, Wifi } from 'lucide-react';
import { Tooltip, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneManager } from '../../../hooks/useSceneManager';
import { useToolbarModeContext } from '@/flow_chat/components/toolbar-mode/ToolbarModeContext';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useNotification } from '@/shared/notification-system';
import NotificationButton from '../../TitleBar/NotificationButton';
import { AboutDialog } from '../../AboutDialog';
import { RemoteConnectDialog } from '../../RemoteConnectDialog';

const REMOTE_CONNECT_DISCLAIMER_KEY = 'bitfun:remote-connect:disclaimer-agreed:v1';

const getRemoteDisclaimerAgreed = (): boolean => {
  try {
    return localStorage.getItem(REMOTE_CONNECT_DISCLAIMER_KEY) === 'true';
  } catch {
    return false;
  }
};

const PersistentFooterActions: React.FC = () => {
  const { t } = useI18n('common');
  const { openScene } = useSceneManager();
  const { enableToolbarMode } = useToolbarModeContext();
  const { hasWorkspace } = useCurrentWorkspace();
  const { warning } = useNotification();

  const [menuOpen, setMenuOpen] = useState(false);
  const [menuClosing, setMenuClosing] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [showRemoteConnect, setShowRemoteConnect] = useState(false);
  const [showRemoteDisclaimer, setShowRemoteDisclaimer] = useState(false);
  const [hasAgreedRemoteDisclaimer, setHasAgreedRemoteDisclaimer] = useState<boolean>(() => getRemoteDisclaimerAgreed());

  const closeMenu = useCallback(() => {
    setMenuClosing(true);
    setTimeout(() => {
      setMenuOpen(false);
      setMenuClosing(false);
    }, 150);
  }, []);

  const toggleMenu = () => {
    if (menuOpen) {
      closeMenu();
    } else {
      setMenuOpen(true);
    }
  };

  const handleOpenSettings = () => {
    closeMenu();
    openScene('settings');
  };

  const handleShowAbout = () => {
    closeMenu();
    setShowAbout(true);
  };

  const handleFloatingMode = () => {
    closeMenu();
    enableToolbarMode();
  };

  const handleRemoteConnect = useCallback(async () => {
    if (!hasWorkspace) {
      warning(t('header.remoteConnectRequiresWorkspace'));
      return;
    }

    closeMenu();

    if (hasAgreedRemoteDisclaimer || getRemoteDisclaimerAgreed()) {
      setHasAgreedRemoteDisclaimer(true);
      setShowRemoteConnect(true);
      return;
    }

    setShowRemoteDisclaimer(true);
  }, [hasWorkspace, warning, t, closeMenu, hasAgreedRemoteDisclaimer]);

  const handleAgreeDisclaimer = useCallback(() => {
    try {
      localStorage.setItem(REMOTE_CONNECT_DISCLAIMER_KEY, 'true');
    } catch {
      // Ignore storage failures and keep in-memory consent for current session.
    }
    setHasAgreedRemoteDisclaimer(true);
    setShowRemoteDisclaimer(false);
    setShowRemoteConnect(true);
  }, []);

  return (
    <div className="bitfun-nav-panel__footer">
      <div className="bitfun-nav-panel__footer-more-wrap">
        <Tooltip content={t('nav.moreOptions')} placement="right" followCursor disabled={menuOpen}>
          <button
            type="button"
            className={`bitfun-nav-panel__footer-btn bitfun-nav-panel__footer-btn--icon${menuOpen ? ' is-active' : ''}`}
            aria-label={t('nav.moreOptions')}
            aria-expanded={menuOpen}
            onClick={toggleMenu}
          >
            <MoreVertical size={15} />
          </button>
        </Tooltip>

        {menuOpen && (
          <>
            <div
              className="bitfun-nav-panel__footer-backdrop"
              onClick={closeMenu}
            />
            <div
              className={`bitfun-nav-panel__footer-menu${menuClosing ? ' is-closing' : ''}`}
              role="menu"
            >
              <Tooltip
                content={t('header.remoteConnectRequiresWorkspace')}
                placement="right"
                disabled={hasWorkspace}
              >
                <button
                  type="button"
                  className={`bitfun-nav-panel__footer-menu-item${!hasWorkspace ? ' is-disabled' : ''}`}
                  role="menuitem"
                  aria-disabled={!hasWorkspace}
                  onClick={handleRemoteConnect}
                >
                  <Wifi size={14} />
                  <span>{t('header.remoteConnect')}</span>
                </button>
              </Tooltip>
              <div className="bitfun-nav-panel__footer-menu-divider" />
              <button
                type="button"
                className="bitfun-nav-panel__footer-menu-item"
                role="menuitem"
                onClick={handleFloatingMode}
              >
                <PictureInPicture2 size={14} />
                <span>{t('header.switchToToolbar')}</span>
              </button>
              <div className="bitfun-nav-panel__footer-menu-divider" />
              <button
                type="button"
                className="bitfun-nav-panel__footer-menu-item"
                role="menuitem"
                onClick={handleOpenSettings}
              >
                <Settings size={14} />
                <span>{t('tabs.settings')}</span>
              </button>
              <button
                type="button"
                className="bitfun-nav-panel__footer-menu-item"
                role="menuitem"
                onClick={handleShowAbout}
              >
                <Info size={14} />
                <span>{t('header.about')}</span>
              </button>
            </div>
          </>
        )}
      </div>

      <NotificationButton className="bitfun-nav-panel__footer-btn" />
      <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
      <RemoteConnectDialog isOpen={showRemoteConnect} onClose={() => setShowRemoteConnect(false)} />
      <Modal
        isOpen={showRemoteDisclaimer}
        onClose={() => setShowRemoteDisclaimer(false)}
        title={t('remoteConnect.disclaimerTitle')}
        showCloseButton
        size="large"
        contentInset
      >
        <div className="bitfun-nav-panel__remote-disclaimer">
          <p className="bitfun-nav-panel__remote-disclaimer-text">{t('remoteConnect.disclaimerIntro')}</p>
          <ol className="bitfun-nav-panel__remote-disclaimer-list">
            <li>{t('remoteConnect.disclaimerItemBeta')}</li>
            <li>{t('remoteConnect.disclaimerItemSecurity')}</li>
            <li>{t('remoteConnect.disclaimerItemEncryption')}</li>
            <li>{t('remoteConnect.disclaimerItemOpenSource')}</li>
            <li>{t('remoteConnect.disclaimerItemPrivacy')}</li>
            <li>{t('remoteConnect.disclaimerItemDataUsage')}</li>
            <li>{t('remoteConnect.disclaimerItemCredentials')}</li>
            <li>{t('remoteConnect.disclaimerItemQrCode')}</li>
            <li>{t('remoteConnect.disclaimerItemNgrok')}</li>
            <li>{t('remoteConnect.disclaimerItemSelfHosted')}</li>
            <li>{t('remoteConnect.disclaimerItemNetwork')}</li>
            <li>{t('remoteConnect.disclaimerItemBot')}</li>
            <li>{t('remoteConnect.disclaimerItemBotPersistence')}</li>
            <li>{t('remoteConnect.disclaimerItemMobileBrowser')}</li>
            <li>{t('remoteConnect.disclaimerItemCompliance')}</li>
            <li>{t('remoteConnect.disclaimerItemLiability')}</li>
          </ol>

          <div className="bitfun-nav-panel__remote-disclaimer-actions">
            <button
              type="button"
              className="bitfun-nav-panel__remote-disclaimer-btn bitfun-nav-panel__remote-disclaimer-btn--secondary"
              onClick={() => setShowRemoteDisclaimer(false)}
            >
              {t('remoteConnect.disclaimerDecline')}
            </button>
            <button
              type="button"
              className="bitfun-nav-panel__remote-disclaimer-btn bitfun-nav-panel__remote-disclaimer-btn--primary"
              onClick={handleAgreeDisclaimer}
            >
              {t('remoteConnect.disclaimerAgree')}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
};

export default PersistentFooterActions;

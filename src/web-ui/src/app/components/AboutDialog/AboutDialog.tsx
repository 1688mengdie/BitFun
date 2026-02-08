/**
 * About dialog component.
 * Shows app version and license info.
 */

import React, { useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Tooltip } from '@/component-library';
import { 
  X, 
  Copy, 
  Check
} from 'lucide-react';
import { 
  getAboutInfo, 
  formatVersion, 
  formatBuildDate
} from '@/shared/utils/version';
import { CubeIcon } from '../Header/CubeIcon';
import { createLogger } from '@/shared/utils/logger';
import './AboutDialog.scss';

const log = createLogger('AboutDialog');

interface AboutDialogProps {
  /** Whether visible */
  isOpen: boolean;
  /** Close callback */
  onClose: () => void;
}

export const AboutDialog: React.FC<AboutDialogProps> = ({
  isOpen,
  onClose
}) => {
  const { t } = useI18n('common');
  const [copiedItem, setCopiedItem] = useState<string | null>(null);
  
  const aboutInfo = getAboutInfo();
  const { version, license } = aboutInfo;

  // Copy to clipboard
  const copyToClipboard = async (text: string, itemId: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedItem(itemId);
      setTimeout(() => setCopiedItem(null), 2000);
    } catch (err) {
      log.error('Failed to copy to clipboard', err);
    }
  };
  
  if (!isOpen) return null;
  
  return (
    <div className="bitfun-about-overlay" onClick={onClose}>
      <div className="bitfun-about-dialog" onClick={(e) => e.stopPropagation()}>
        {/* Close button */}
        <Tooltip content={t('about.close')}>
          <button 
            className="bitfun-about-dialog__close"
            onClick={onClose}
          >
            <X size={16} />
          </button>
        </Tooltip>

        {/* Content */}
        <div className="bitfun-about-dialog__content">
          {/* Hero section - cube + product info */}
          <div className="bitfun-about-dialog__hero">
            {/* Cube logo */}
            <div className="bitfun-about-dialog__cube-wrapper">
              <CubeIcon size={72} className="bitfun-about-dialog__cube-icon" />
            </div>
            <h1 className="bitfun-about-dialog__title">{version.name}</h1>
            <div className="bitfun-about-dialog__version-badge">
              {t('about.version', { version: formatVersion(version.version, version.isDev) })}
            </div>
            {/* Decorative divider */}
            <div className="bitfun-about-dialog__divider"></div>
            {/* Decorative dots */}
            <div className="bitfun-about-dialog__dots">
              <span></span>
              <span></span>
              <span></span>
            </div>
          </div>

          {/* Scrollable area */}
          <div className="bitfun-about-dialog__scrollable">
            {/* Version info card */}
            <div className="bitfun-about-dialog__info-section">
              <div className="bitfun-about-dialog__info-card">
                <div className="bitfun-about-dialog__info-row">
                  <span className="bitfun-about-dialog__info-label">{t('about.buildDate')}</span>
                  <span className="bitfun-about-dialog__info-value">
                    {formatBuildDate(version.buildDate)}
                  </span>
                </div>
                
                {version.gitCommit && (
                  <div className="bitfun-about-dialog__info-row">
                    <span className="bitfun-about-dialog__info-label">{t('about.commit')}</span>
                    <div className="bitfun-about-dialog__info-value-group">
                      <span className="bitfun-about-dialog__info-value bitfun-about-dialog__info-value--mono">
                        {version.gitCommit}
                      </span>
                      <Tooltip content={t('about.copy')}>
                        <button
                          className="bitfun-about-dialog__copy-btn"
                          onClick={() => copyToClipboard(version.gitCommit || '', 'commit')}
                        >
                          {copiedItem === 'commit' ? <Check size={12} /> : <Copy size={12} />}
                        </button>
                      </Tooltip>
                    </div>
                  </div>
                )}
                
                {version.gitBranch && (
                  <div className="bitfun-about-dialog__info-row">
                    <span className="bitfun-about-dialog__info-label">{t('about.branch')}</span>
                    <span className="bitfun-about-dialog__info-value">{version.gitBranch}</span>
                  </div>
                )}
              </div>
            </div>

          </div>

          {/* Footer */}
          <div className="bitfun-about-dialog__footer">
            <p className="bitfun-about-dialog__license">{license.text}</p>
            <p className="bitfun-about-dialog__copyright">
              {t('about.copyright')}
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

export default AboutDialog;

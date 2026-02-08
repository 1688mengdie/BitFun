/**
 * Scroll-to-latest bar.
 * Minimal divider style with a soft fade.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import './ScrollToLatestBar.scss';

interface ScrollToLatestBarProps {
  visible: boolean;
  onClick: () => void;
  /** Whether ChatInput is expanded. */
  isInputExpanded?: boolean;
  /** Whether ChatInput is active. */
  isInputActive?: boolean;
  className?: string;
}

export const ScrollToLatestBar: React.FC<ScrollToLatestBarProps> = ({
  visible,
  onClick,
  isInputExpanded = false,
  isInputActive = true,
  className = ''
}) => {
  const { t } = useTranslation('flow-chat');
  
  if (!visible) return null;

  // Derive the modifier class from ChatInput state.
  const inputStateClass = !isInputActive 
    ? 'scroll-to-latest-bar--input-collapsed'
    : isInputExpanded 
      ? 'scroll-to-latest-bar--input-expanded' 
      : '';

  return (
    <div 
      className={`scroll-to-latest-bar ${inputStateClass} ${className}`}
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onClick();
        }
      }}
      aria-label={t('scroll.toLatest')}
    >
      <div className="scroll-to-latest-bar__gradient" />
      
      <div className="scroll-to-latest-bar__content">
        <span className="scroll-to-latest-bar__line" />
        <span className="scroll-to-latest-bar__text">{t('scroll.clickToLatest')}</span>
        <span className="scroll-to-latest-bar__line" />
      </div>
    </div>
  );
};

ScrollToLatestBar.displayName = 'ScrollToLatestBar';

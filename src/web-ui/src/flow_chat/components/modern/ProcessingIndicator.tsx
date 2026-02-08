/**
 * Processing indicator.
 * Shows pulsing dots while the session is processing.
 * reserveSpace keeps layout height even when hidden.
 */

import React from 'react';
import './ProcessingIndicator.scss';

interface ProcessingIndicatorProps {
  visible: boolean;
  /** When true, preserve height to avoid layout jumps. */
  reserveSpace?: boolean;
}

export const ProcessingIndicator: React.FC<ProcessingIndicatorProps> = ({ visible, reserveSpace = false }) => {
  const shouldRender = visible || reserveSpace;
  if (!shouldRender) return null;

  return (
    <div className="processing-indicator" aria-hidden={!visible}>
      <div
        className="processing-indicator__content"
        style={visible ? undefined : { visibility: 'hidden' as const }}
      >
        <div className="processing-indicator__dots">
          <div className="processing-indicator__dot processing-indicator__dot--1" />
          <div className="processing-indicator__dot processing-indicator__dot--2" />
          <div className="processing-indicator__dot processing-indicator__dot--3" />
        </div>
      </div>
    </div>
  );
};


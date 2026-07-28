import React from 'react';

export interface ConfigStatusProps {
  status: 'success' | 'error' | 'warning' | 'info';
  message?: string;
  className?: string;
}

export const ConfigStatus: React.FC<ConfigStatusProps> = ({ status, message, className }) => (
  <div className={`${className || ''} config-status config-status--${status}`}>
    {message && <span>{message}</span>}
  </div>
);

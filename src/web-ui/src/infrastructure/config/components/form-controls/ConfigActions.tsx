import React from 'react';

export interface ConfigActionsProps {
  children?: React.ReactNode;
  className?: string;
}

export const ConfigActions: React.FC<ConfigActionsProps> = ({ children, className }) => (
  <div className={className}>{children}</div>
);

export interface ConfigActionButtonsProps {
  children?: React.ReactNode;
  className?: string;
}

export const ConfigActionButtons: React.FC<ConfigActionButtonsProps> = ({ children, className }) => (
  <div className={className}>{children}</div>
);

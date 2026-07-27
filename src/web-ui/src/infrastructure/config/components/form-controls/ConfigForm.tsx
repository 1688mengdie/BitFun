import React from 'react';

export interface ConfigFormProps {
  onSubmit?: () => void;
  children?: React.ReactNode;
  className?: string;
}

export const ConfigForm: React.FC<ConfigFormProps> = ({ onSubmit, children, className }) => (
  <form
    onSubmit={(e) => {
      e.preventDefault();
      onSubmit?.();
    }}
    className={className}
  >
    {children}
  </form>
);

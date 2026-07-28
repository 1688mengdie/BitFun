import React from 'react';

export interface ConfigSectionProps {
  title?: string;
  description?: string;
  children?: React.ReactNode;
  className?: string;
}

export const ConfigSection: React.FC<ConfigSectionProps> = ({ title, description, children, className }) => (
  <div className={className}>
    {title && <h4>{title}</h4>}
    {description && <p>{description}</p>}
    {children}
  </div>
);

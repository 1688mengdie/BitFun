 

import React from 'react';
import './ConfigPageLayout.scss';

export interface ConfigPageLayoutProps {
   
  children: React.ReactNode;
   
  className?: string;
}

 
export const ConfigPageLayout: React.FC<ConfigPageLayoutProps> = ({
  children,
  className = '',
}) => {
  return (
    <div className={`bitfun-config-page-layout ${className}`}>
      {children}
    </div>
  );
};

export interface ConfigPageContentProps {
   
  children: React.ReactNode;
   
  className?: string;
}

 
export const ConfigPageContent: React.FC<ConfigPageContentProps> = ({
  children,
  className = '',
}) => {
  return (
    <div className={`bitfun-config-page-content ${className}`}>
      {children}
    </div>
  );
};

export default ConfigPageLayout;




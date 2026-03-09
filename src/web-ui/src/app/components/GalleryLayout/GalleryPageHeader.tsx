import React from 'react';

interface GalleryPageHeaderProps {
  title: string;
  subtitle?: React.ReactNode;
  actions?: React.ReactNode;
  extraContent?: React.ReactNode;
}

const GalleryPageHeader: React.FC<GalleryPageHeaderProps> = ({
  title,
  subtitle,
  actions,
  extraContent,
}) => (
  <div className="gallery-page-header">
    <div className="gallery-page-header__identity">
      <h2 className="gallery-page-header__title">{title}</h2>
      {subtitle ? <div className="gallery-page-header__subtitle">{subtitle}</div> : null}
      {extraContent ? <div className="gallery-page-header__extra">{extraContent}</div> : null}
    </div>
    {actions ? <div className="gallery-page-header__actions">{actions}</div> : null}
  </div>
);

export default GalleryPageHeader;
export type { GalleryPageHeaderProps };

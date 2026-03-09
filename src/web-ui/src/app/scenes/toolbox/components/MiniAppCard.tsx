import React from 'react';
import { Play, Square, Trash2 } from 'lucide-react';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { renderMiniAppIcon } from '../utils/miniAppIcons';
import './MiniAppCard.scss';

interface MiniAppCardProps {
  app: MiniAppMeta;
  index?: number;
  isRunning?: boolean;
  onOpenDetails: (app: MiniAppMeta) => void;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onStop?: (id: string) => void;
}

const MiniAppCard: React.FC<MiniAppCardProps> = ({
  app,
  index = 0,
  isRunning = false,
  onOpenDetails,
  onOpen,
  onDelete,
  onStop,
}) => {
  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete(app.id);
  };

  const handleStopClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onStop?.(app.id);
  };

  const handleOpenClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onOpen(app.id);
  };

  const handleOpenDetails = () => {
    onOpenDetails(app);
  };

  return (
    <div
      className={[
        'miniapp-card',
        isRunning && 'miniapp-card--running',
      ]
        .filter(Boolean)
        .join(' ')}
      style={{ '--card-index': index } as React.CSSProperties}
      onClick={handleOpenDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && handleOpenDetails()}
      aria-label={app.name}
    >
      <div className="miniapp-card__icon-area">
        <div className="miniapp-card__icon">
          {renderMiniAppIcon(app.icon || 'box', 28)}
        </div>
      </div>

      <div className="miniapp-card__body">
        <div className="miniapp-card__row">
          <span className="miniapp-card__name">{app.name}</span>
          {isRunning && <span className="miniapp-card__run-dot" />}
          <span className="miniapp-card__version">v{app.version}</span>
          <div className="miniapp-card__actions">
            <button
              className="miniapp-card__action-btn miniapp-card__action-btn--primary"
              onClick={handleOpenClick}
              aria-label="启动"
              title="启动"
            >
              <Play size={15} fill="currentColor" strokeWidth={0} />
            </button>
            {isRunning && onStop ? (
              <button
                className="miniapp-card__action-btn miniapp-card__action-btn--stop"
                onClick={handleStopClick}
                aria-label="停止"
                title="停止"
              >
                <Square size={13} />
              </button>
            ) : (
              <button
                className="miniapp-card__action-btn miniapp-card__action-btn--danger"
                onClick={handleDeleteClick}
                aria-label="删除"
                title="删除"
              >
                <Trash2 size={13} />
              </button>
            )}
          </div>
        </div>
        {app.description ? (
          <div className="miniapp-card__desc">
            <span className="miniapp-card__desc-inner">{app.description}</span>
          </div>
        ) : null}
        {app.tags.length > 0 ? (
          <div className="miniapp-card__tags">
            {app.tags.slice(0, 3).map((tag) => (
              <span key={tag} className="miniapp-card__tag">{tag}</span>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
};

export default MiniAppCard;

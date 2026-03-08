import React from 'react';
import { Package, Puzzle } from 'lucide-react';
import { getCardGradient } from '@/shared/utils/cardGradients';
import './SkillCard.scss';

type SkillCardActionTone = 'primary' | 'danger' | 'success' | 'muted';

export interface SkillCardAction {
  id: string;
  icon: React.ReactNode;
  ariaLabel: string;
  title?: string;
  disabled?: boolean;
  tone?: SkillCardActionTone;
  onClick: () => void;
}

interface SkillCardProps {
  name: string;
  description?: string;
  index?: number;
  accentSeed?: string;
  iconKind?: 'skill' | 'market';
  badges?: React.ReactNode;
  meta?: React.ReactNode;
  actions?: SkillCardAction[];
  onOpenDetails?: () => void;
}

const SkillCard: React.FC<SkillCardProps> = ({
  name,
  description,
  index = 0,
  accentSeed,
  iconKind = 'skill',
  badges,
  meta,
  actions = [],
  onOpenDetails,
}) => {
  const Icon = iconKind === 'market' ? Package : Puzzle;
  const openDetails = () => onOpenDetails?.();

  return (
    <div
      className="skill-card"
      style={{
        '--card-index': index,
        '--skill-card-gradient': getCardGradient(accentSeed ?? name),
      } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && openDetails()}
      aria-label={name}
    >
      <div className="skill-card__icon-area">
        <div className="skill-card__icon">
          <Icon size={26} strokeWidth={1.6} />
        </div>
      </div>

      <div className="skill-card__body">
        <div className="skill-card__header">
          <div className="skill-card__header-text">
            <span className="skill-card__name">{name}</span>
            {badges}
          </div>
          {actions.length > 0 && (
            <div className="skill-card__actions" onClick={(e) => e.stopPropagation()}>
              {actions.map((action) => (
                <button
                  key={action.id}
                  type="button"
                  className={[
                    'skill-card__action-btn',
                    action.tone && `skill-card__action-btn--${action.tone}`,
                  ].filter(Boolean).join(' ')}
                  onClick={action.onClick}
                  disabled={action.disabled}
                  aria-label={action.ariaLabel}
                  title={action.title ?? action.ariaLabel}
                >
                  {action.icon}
                </button>
              ))}
            </div>
          )}
        </div>

        {description?.trim() && (
          <p className="skill-card__desc">{description.trim()}</p>
        )}

        {meta && <div className="skill-card__meta">{meta}</div>}
      </div>
    </div>
  );
};

export default SkillCard;

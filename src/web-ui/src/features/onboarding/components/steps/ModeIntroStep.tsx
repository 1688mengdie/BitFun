/**
 * Mode intro step
 * ModeIntroStep - minimal, premium mode overview
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Zap, GitBranch, Search } from 'lucide-react';

interface ModeIntroStepProps {}

const MODE_LIST = [
  {
    id: 'agentic',
    icon: Zap,
    nameKey: 'modes.modeList.agentic.name',
    descKey: 'modes.modeList.agentic.description',
    featuresKey: 'modes.modeList.agentic.features'
  },
  {
    id: 'plan',
    icon: GitBranch,
    nameKey: 'modes.modeList.plan.name',
    descKey: 'modes.modeList.plan.description',
    featuresKey: 'modes.modeList.plan.features'
  },
  {
    id: 'debug',
    icon: Search,
    nameKey: 'modes.modeList.debug.name',
    descKey: 'modes.modeList.debug.description',
    featuresKey: 'modes.modeList.debug.features'
  }
];

export const ModeIntroStep: React.FC<ModeIntroStepProps> = () => {
  const { t } = useTranslation('onboarding');

  return (
    <div className="bitfun-onboarding-step bitfun-onboarding-modes">
      {/* Title */}
      <div className="bitfun-onboarding-step__header">
        <h1 className="bitfun-onboarding-step__title">
          {t('modes.title')}
        </h1>
        <p className="bitfun-onboarding-step__description">
          {t('modes.description')}
        </p>
      </div>

      {/* Mode list */}
      <div className="bitfun-onboarding-modes__grid">
        {MODE_LIST.map((mode) => {
          const IconComponent = mode.icon;
          const features = t(mode.featuresKey, { returnObjects: true }) as string[];
          
          return (
            <div key={mode.id} className={`bitfun-onboarding-modes__item bitfun-onboarding-modes__item--${mode.id}`}>
              <div className="bitfun-onboarding-modes__item-icon">
                <IconComponent size={20} />
              </div>
              <div className="bitfun-onboarding-modes__item-name">
                {t(mode.nameKey)}
              </div>
              <div className="bitfun-onboarding-modes__item-desc">
                {t(mode.descKey)}
              </div>
              <div className="bitfun-onboarding-modes__item-features">
                {Array.isArray(features) && features.slice(0, 2).map((feature, idx) => (
                  <span key={idx} className="bitfun-onboarding-modes__item-feature">
                    {feature}
                  </span>
                ))}
              </div>
            </div>
          );
        })}
      </div>

      {/* Tip */}
      <p className="bitfun-onboarding-modes__tip">
        {t('modes.tip')}
      </p>
    </div>
  );
};

export default ModeIntroStep;

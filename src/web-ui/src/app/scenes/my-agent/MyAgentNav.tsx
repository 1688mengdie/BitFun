import React, { useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useApp } from '@/app/hooks/useApp';
import { MY_AGENT_NAV_CATEGORIES } from './myAgentConfig';
import type { MyAgentView } from './myAgentConfig';
import { useMyAgentStore } from './myAgentStore';
import './MyAgentNav.scss';

const MyAgentNav: React.FC = () => {
  const { t } = useI18n('common');
  const activeView = useMyAgentStore((s) => s.activeView);
  const setActiveView = useMyAgentStore((s) => s.setActiveView);
  const { switchLeftPanelTab } = useApp();

  const handleItemClick = useCallback((view: MyAgentView, panelTab: Parameters<typeof switchLeftPanelTab>[0]) => {
    setActiveView(view);
    switchLeftPanelTab(panelTab);
  }, [setActiveView, switchLeftPanelTab]);

  return (
    <div className="bitfun-my-agent-nav">
      <div className="bitfun-my-agent-nav__header">
        <span className="bitfun-my-agent-nav__title">{t('nav.myAgent.title')}</span>
      </div>

      <div className="bitfun-my-agent-nav__sections">
        {MY_AGENT_NAV_CATEGORIES.map((category) => (
          <div key={category.id} className="bitfun-my-agent-nav__category">
            <div className="bitfun-my-agent-nav__category-header">
              <span className="bitfun-my-agent-nav__category-label">
                {t(category.nameKey)}
              </span>
            </div>

            <div className="bitfun-my-agent-nav__items">
              {category.items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={[
                    'bitfun-my-agent-nav__item',
                    activeView === item.id && 'is-active',
                  ].filter(Boolean).join(' ')}
                  onClick={() => handleItemClick(item.id, item.panelTab)}
                >
                  {t(item.labelKey)}
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default MyAgentNav;

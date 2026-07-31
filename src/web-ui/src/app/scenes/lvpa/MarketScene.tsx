import React from 'react';
import { CardMarket } from './CardMarket';
import './MarketScene.scss';

export const MarketScene: React.FC<{ isActive?: boolean }> = () => {
  return (
    <div className="lvpa-scene-market" data-scene="lvpa-market">
      <CardMarket />
    </div>
  );
};

export default MarketScene;

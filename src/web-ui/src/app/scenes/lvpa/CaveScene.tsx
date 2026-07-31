import React from 'react';
import { CultivatorProfile } from './CultivatorProfile';
import { CardSlots } from './CardSlots';
import './CaveScene.scss';

export const CaveScene: React.FC<{ isActive?: boolean }> = () => {
  return (
    <div className="lvpa-scene-cave" data-scene="lvpa-cave">
      <CultivatorProfile />
      <CardSlots />
    </div>
  );
};

export default CaveScene;

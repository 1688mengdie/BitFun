import React from 'react';
import { SectMap } from './SectMap';
import './SectScene.scss';

export const SectScene: React.FC<{ isActive?: boolean }> = () => {
  return (
    <div className="lvpa-scene-sect" data-scene="lvpa-sect">
      <SectMap />
    </div>
  );
};

export default SectScene;

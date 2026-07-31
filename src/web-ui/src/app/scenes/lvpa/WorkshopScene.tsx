import React from 'react';
import { WorkshopDAG } from './WorkshopDAG';
import './WorkshopScene.scss';

export const WorkshopScene: React.FC<{ isActive?: boolean }> = () => {
  return (
    <div className="lvpa-scene-workshop" data-scene="lvpa-workshop">
      <WorkshopDAG />
    </div>
  );
};

export default WorkshopScene;

import React from 'react';
import { PartyLobby } from './PartyLobby';
import './GateScene.scss';

export const GateScene: React.FC<{ isActive?: boolean }> = () => {
  return (
    <div className="lvpa-scene-gate" data-scene="lvpa-gate">
      <PartyLobby />
    </div>
  );
};

export default GateScene;

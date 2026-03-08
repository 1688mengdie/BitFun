import React, { Suspense, lazy } from 'react';
import { useMyAgentStore } from './myAgentStore';
import './MyAgentScene.scss';

const ProfileScene = lazy(() => import('../profile/ProfileScene'));
const AgentsScene = lazy(() => import('../agents/AgentsScene'));
const SkillsScene = lazy(() => import('../skills/SkillsScene'));

interface MyAgentSceneProps {
  workspacePath?: string;
}

const MyAgentScene: React.FC<MyAgentSceneProps> = ({ workspacePath }) => {
  const activeView = useMyAgentStore((s) => s.activeView);

  return (
    <div className="bitfun-my-agent-scene">
      <Suspense fallback={<div className="bitfun-my-agent-scene__loading" />}>
        {activeView === 'profile' && <ProfileScene workspacePath={workspacePath} />}
        {activeView === 'agents' && <AgentsScene />}
        {activeView === 'skills' && <SkillsScene />}
      </Suspense>
    </div>
  );
};

export default MyAgentScene;

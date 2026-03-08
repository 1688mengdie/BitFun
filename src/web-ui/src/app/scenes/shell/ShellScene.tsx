import React, { Suspense, lazy } from 'react';
import './ShellScene.scss';

const TerminalScene = lazy(() => import('../terminal/TerminalScene'));

const ShellScene: React.FC = () => (
  <div className="bitfun-shell-scene">
    <Suspense fallback={<div className="bitfun-shell-scene__loading" />}>
      <TerminalScene />
    </Suspense>
  </div>
);

export default ShellScene;

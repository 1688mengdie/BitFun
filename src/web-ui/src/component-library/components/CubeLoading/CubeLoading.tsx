/**
 * CubeLoading - 3x3x3 cube loading animation
 */

import React from 'react';
import './CubeLoading.scss';

export type CubeLoadingSize = 'small' | 'medium' | 'large';

export interface CubeLoadingProps {
  /** Size: small(32px) | medium(48px) | large(72px) */
  size?: CubeLoadingSize;
  /** Loading text */
  text?: string;
  /** Custom class name */
  className?: string;
}

const BLOCKS = (() => {
  const arr = [];
  for (let x = -1; x <= 1; x++) {
    for (let y = -1; y <= 1; y++) {
      for (let z = -1; z <= 1; z++) {
        arr.push({ x, y, z });
      }
    }
  }
  return arr;
})();

export const CubeLoading: React.FC<CubeLoadingProps> = ({
  size = 'medium',
  text,
  className = '',
}) => {
  return (
    <div className={`cube-loading cube-loading--${size} ${className}`}>
      <div className="cube-loading__scene">
        <div className="cube-loading__rubiks">
          {BLOCKS.map(({ x, y, z }, i) => (
            <div
              key={i}
              className="cube-loading__block"
              style={{
                transform: `translate3d(
                  calc(var(--unit) * ${x}),
                  calc(var(--unit) * ${-y}),
                  calc(var(--unit) * ${z})
                )`,
              }}
            >
              <div className="cube-loading__face cube-loading__face--front" />
              <div className="cube-loading__face cube-loading__face--back" />
              <div className="cube-loading__face cube-loading__face--top" />
              <div className="cube-loading__face cube-loading__face--bottom" />
              <div className="cube-loading__face cube-loading__face--right" />
              <div className="cube-loading__face cube-loading__face--left" />
            </div>
          ))}
        </div>
      </div>
      {text && <div className="cube-loading__text">{text}</div>}
    </div>
  );
};

CubeLoading.displayName = 'CubeLoading';

export default CubeLoading;

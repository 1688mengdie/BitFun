/**
 * Virtual item renderer.
 * Renders user messages, model rounds, or explore groups by type.
 */

import React from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import { UserMessageItem } from './UserMessageItem';
import { ModelRoundItem } from './ModelRoundItem';
import { ExploreGroupRenderer } from './ExploreGroupRenderer';
import './VirtualItemRenderer.scss';

interface VirtualItemRendererProps {
  item: VirtualItem;
  index: number;
}

export const VirtualItemRenderer = React.memo<VirtualItemRendererProps>(
  ({ item, index }) => {
    const content = (() => {
      switch (item.type) {
        case 'user-message':
          return <UserMessageItem message={item.data} turnId={item.turnId} />;
        
        case 'model-round':
          return (
            <ModelRoundItem 
              round={item.data} 
              turnId={item.turnId} 
              isLastRound={item.isLastRound}
            />
          );
        
        case 'explore-group':
          return (
            <ExploreGroupRenderer
              data={item.data}
              turnId={item.turnId}
            />
          );
        
        default:
          // Return a placeholder instead of null to avoid zero-size errors.
          return <div style={{ minHeight: '1px' }} />;
      }
    })();
    
    // A4-like layout: wrap with a max-width container.
    // Render the container even when content is empty to avoid zero-size issues.
    // data-turn-id is used for long-image export.
    return (
      <div 
        className="virtual-item-wrapper" 
        data-turn-id={item.turnId}
        data-item-type={item.type}
      >
        {content || <div style={{ minHeight: '1px' }} />}
      </div>
    );
  },
  (prev, next) => (
    prev.item === next.item &&
    prev.index === next.index
  )
);

VirtualItemRenderer.displayName = 'VirtualItemRenderer';


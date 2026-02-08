/**
 * Display component for MCP tools.
 */

import React, { useState, useCallback } from 'react';
import { ChevronDown, ChevronUp, Package, CheckCircle, Check, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { CubeLoading, IconButton } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { createLogger } from '@/shared/utils/logger';
import './MCPToolDisplay.scss';

const log = createLogger('MCPToolDisplay');

interface MCPToolResultContent {
  type: 'text' | 'image' | 'resource';
  text?: string;
  data?: string;
  mime_type?: string;
  resource?: {
    uri: string;
    name?: string;
    description?: string;
    mime_type?: string;
  };
}

interface MCPToolResult {
  content?: MCPToolResultContent[];
  is_error?: boolean;
}

export const MCPToolDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  config,
  onConfirm,
  onReject
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult, requiresConfirmation, userConfirmed } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);

  const getResultData = (): MCPToolResult | null => {
    if (!toolResult?.result) return null;
    
    try {
      if (typeof toolResult.result === 'string') {
        return JSON.parse(toolResult.result);
      }
      return toolResult.result as MCPToolResult;
    } catch (e) {
      log.error('Failed to parse MCP tool result', e);
      return null;
    }
  };

  const resultData = getResultData();

  const getToolInfo = () => {
    const fullToolName = config.toolName;
    const parts = fullToolName.split('_');
    const actualToolName = parts.slice(2).join('_') || fullToolName;
    const serverName = parts[1] || 'unknown';
    
    return { toolName: actualToolName, serverName };
  };

  const { toolName, serverName } = getToolInfo();

  const getContentSummary = () => {
    if (!resultData?.content) return null;
    
    const counts = {
      text: 0,
      image: 0,
      resource: 0
    };
    
    resultData.content.forEach(item => {
      if (item.type in counts) {
        counts[item.type as keyof typeof counts]++;
      }
    });
    
    const parts = [];
    if (counts.text > 0) parts.push(`${counts.text} text`);
    if (counts.image > 0) parts.push(`${counts.image} images`);
    if (counts.resource > 0) parts.push(`${counts.resource} resources`);
    
    return parts.length > 0 ? parts.join(' · ') : null;
  };

  const contentSummary = getContentSummary();
  const hasContent = status === 'completed' && (resultData?.content && resultData.content.length > 0);
  
  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';

  const isFailed = status === 'error';

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult) {
      return toolResult.error;
    }
    return 'MCP tool execution failed';
  };

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.preview-toggle-btn')) {
      return;
    }
    
    if (isFailed) {
      return;
    }
    
    if (hasContent) {
      setIsExpanded(!isExpanded);
    }
  }, [isFailed, isExpanded, hasContent]);

  const renderToolIcon = () => {
    return <Package size={16} />;
  };

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    if (status === 'completed' && !isFailed) {
      return <CheckCircle className="icon-completed" size={14} />;
    }
    return null;
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={renderToolIcon()}
      iconClassName="mcp-icon"
      action={isFailed ? 'MCP tool failed' : 'MCP tool:'}
      content={
        <span className="mcp-tool-info">
          <span className="tool-name">{toolName}</span>
          <span className="server-tag">from {serverName}</span>
        </span>
      }
      extra={
        <>
          {!isFailed && contentSummary && status === 'completed' && (
            <span className="content-summary">
              {contentSummary}
            </span>
          )}
          
          {requiresConfirmation && !userConfirmed && status !== 'completed' && (
            <div className="mcp-action-buttons">
              <IconButton
                className="mcp-icon-button mcp-confirm-btn"
                variant="success"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation();
                  onConfirm?.(toolCall?.input);
                }}
                disabled={status === 'streaming'}
                tooltip={t('toolCards.mcp.confirmExecute')}
              >
                <Check size={14} />
              </IconButton>
              <IconButton
                className="mcp-icon-button mcp-reject-btn"
                variant="danger"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation();
                  onReject?.();
                }}
                disabled={status === 'streaming'}
                tooltip={t('toolCards.mcp.cancel')}
              >
                <X size={14} />
              </IconButton>
            </div>
          )}
          
          {!isFailed && hasContent && (
            <IconButton
              className="preview-toggle-btn"
              variant="ghost"
              size="xs"
              onClick={(e) => {
                e.stopPropagation();
                setIsExpanded(!isExpanded);
              }}
              tooltip={isExpanded ? t('toolCards.common.collapseContent') : t('toolCards.common.expandContent')}
            >
              {isExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            </IconButton>
          )}
          
          {isFailed && (
            <div className="error-expand-indicator">
              <span className="error-text">Failed</span>
            </div>
          )}
        </>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  const renderExpandedContent = () => {
    if (!resultData?.content) {
      return null;
    }

    return (
      <div className="mcp-expanded-content">
        {resultData.content.map((item, index) => (
          <div key={index} className={`content-item content-item-${item.type}`}>
            {item.type === 'text' && (
              <div className="text-content">
                <pre>{item.text}</pre>
              </div>
            )}
            {item.type === 'resource' && item.resource && (
              <div className="resource-content">
                <div className="resource-name">{item.resource.name || 'Resource'}</div>
                <div className="resource-uri">{item.resource.uri}</div>
                {item.resource.description && (
                  <div className="resource-description">{item.resource.description}</div>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
    );
  };

  const renderErrorContent = () => (
    <div className="error-content">
      <div className="error-message">{getErrorMessage()}</div>
      <div className="error-meta">
        <span className="error-tool">Tool: {toolName}</span>
        <span className="error-separator">|</span>
        <span className="error-server">Server: {serverName}</span>
      </div>
    </div>
  );

  return (
    <BaseToolCard
      status={status}
      isExpanded={isExpanded}
      onClick={handleCardClick}
      className="mcp-tool-display"
      header={renderHeader()}
      expandedContent={renderExpandedContent()}
      errorContent={renderErrorContent()}
      isFailed={isFailed}
      requiresConfirmation={requiresConfirmation && !userConfirmed}
    />
  );
};

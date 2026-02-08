/**
 * Markdown component
 * Used to render Markdown-formatted text
 */

import React, { useState, useMemo, useCallback } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus, vs } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { useI18n } from '@/infrastructure/i18n';
import { MermaidBlock } from './MermaidBlock';
import { ReproductionStepsBlock } from './ReproductionStepsBlock';
import { systemAPI } from '../../../infrastructure/api';
import { getPrismLanguageFromAlias } from '@/infrastructure/language-detection';
import { useTheme } from '@/infrastructure/theme';
import { createLogger } from '@/shared/utils/logger';
import './Markdown.scss';

const log = createLogger('Markdown');

const CopyButton: React.FC<{ code: string }> = ({ code }) => {
  const { t } = useI18n('components');
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      log.warn('Failed to copy code', { error });
    }
  };

  return (
    <button 
      className={`copy-button${copied ? ' copy-success' : ''}`}
      onClick={handleCopy}
      title={copied ? t('markdown.copySuccess') : t('markdown.copyCode')}
    >
      {copied ? (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12"></polyline>
        </svg>
      ) : (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
        </svg>
      )}
    </button>
  );
};

export interface LineRange {
  start: number;
  end?: number;
}

export interface MarkdownProps {
  content: string;
  className?: string;
  isStreaming?: boolean;
  onOpenVisualization?: (visualization: any) => void;
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
  onTabOpen?: (tabInfo: any) => void;
  onReproductionProceed?: () => void;
}

export const Markdown = React.memo<MarkdownProps>(({ 
  content, 
  className = '',
  isStreaming = false,
  onOpenVisualization,
  onFileViewRequest,
  onTabOpen,
  onReproductionProceed
}) => {
  const { isLight } = useTheme();
  
  const syntaxTheme = isLight ? vs : vscDarkPlus;
  
  const contentStr = typeof content === 'string' ? content : String(content || '');
  
  // Fault-tolerant extraction of <reproduction_steps> content
  const { markdownContent, reproductionSteps } = useMemo(() => {
    const regex = /<reproduction_steps>([\s\S]*?)<\/reproduction[\s_]*steps\s*>?/g;
    const match = regex.exec(contentStr);
    
    if (match) {
      const steps = match[1].trim();
      const cleanedContent = contentStr.replace(regex, '').trim();
      return { markdownContent: cleanedContent, reproductionSteps: steps };
    }
    
    return { markdownContent: contentStr, reproductionSteps: null };
  }, [contentStr]);
  
  const linkMap = useMemo(() => {
    const map = new Map<string, string>();
    const linkMatches = contentStr.match(/\[([^\]]+)\]\(([^)]+)\)/g) || [];
    
    linkMatches.forEach(match => {
      const linkMatch = match.match(/\[([^\]]+)\]\(([^)]+)\)/);
      if (linkMatch) {
        const [, text, href] = linkMatch;
        map.set(text, href);
      }
    });
    return map;
  }, [contentStr]);
  
  // Parse line ranges like #L42 / 1-20
  const parseLineRange = useCallback((hash: string): LineRange | undefined => {
    const cleanHash = hash.replace(/^#/, '');

    const lineMatchWithL = cleanHash.match(/^L(\d+)(?:-L?(\d+))?$/i);
    if (lineMatchWithL) {
      const start = parseInt(lineMatchWithL[1], 10);
      const end = lineMatchWithL[2] ? parseInt(lineMatchWithL[2], 10) : undefined;
      return { start, end };
    }

    const lineMatchWithoutL = cleanHash.match(/^(\d+)(?:-(\d+))?$/);
    if (lineMatchWithoutL) {
      const start = parseInt(lineMatchWithoutL[1], 10);
      const end = lineMatchWithoutL[2] ? parseInt(lineMatchWithoutL[2], 10) : undefined;
      return { start, end };
    }

    return undefined;
  }, []);

  const handleFileViewRequest = useCallback((filePath: string, fileName: string, lineRange?: LineRange) => {
    onFileViewRequest?.(filePath, fileName, lineRange);
  }, [onFileViewRequest]);

  const handleOpenVisualization = useCallback((visualization: any) => {
    onOpenVisualization?.(visualization);
  }, [onOpenVisualization]);

  const handleTabOpen = useCallback((tabInfo: any) => {
    onTabOpen?.(tabInfo);
  }, [onTabOpen]);
  
  const components = useMemo(() => ({
    code({ node, className, children, ...props }: any) {
      const match = /language-(\w+)/.exec(className || '');
      const language = match ? match[1] : '';
      const code = String(children).replace(/\n$/, '');
      
      const hasMultipleLines = code.includes('\n');
      const isCodeBlock = className?.startsWith('language-') || hasMultipleLines;
      
      if (!isCodeBlock) {
        return (
          <code className="inline-code" {...props}>
            {children}
          </code>
        );
      }
      
      if (language.toLowerCase().startsWith('mermaid')) {
        return (
          <MermaidBlock
            code={code}
            isStreaming={isStreaming}
          />
        );
      }
      
      const normalizedLang = getPrismLanguageFromAlias(language);
      
      return (
        <div className="code-block-wrapper">
          <CopyButton code={code} />
          <SyntaxHighlighter
            language={normalizedLang}
            style={syntaxTheme}
            showLineNumbers={true}
            customStyle={{
              margin: 0,
              borderRadius: '8px',
              fontSize: '0.9rem',
              lineHeight: '1.5'
            }}
            codeTagProps={{
              style: {
                fontFamily: 'var(--markdown-font-mono, "Fira Code", "JetBrains Mono", Consolas, "Courier New", monospace)'
              }
            }}
            lineNumberStyle={{
              color: isLight ? '#999' : '#666',
              paddingRight: '1em',
              textAlign: 'right',
              userSelect: 'none',
              minWidth: '3em'
            }}
          >
            {code}
          </SyntaxHighlighter>
        </div>
      );
    },
    
    a({ node, href, children, ...props }: any) {
      const linkText = typeof children === 'string' ? children : String(children);
      const originalHref = linkMap.get(linkText);
      const hrefValue = originalHref || href || node?.properties?.href;

      if (typeof hrefValue === 'string') {
        let filePath = hrefValue.replace(/^file:/, '');

        let lineRange: LineRange | undefined;
        let fileName: string;

        const hashIndex = filePath.indexOf('#');
        if (hashIndex !== -1) {
          const hash = filePath.substring(hashIndex);
          filePath = filePath.substring(0, hashIndex);
          lineRange = parseLineRange(hash);
        } else {
          // Note: exclude Windows drive letters (e.g. C:)
          const colonMatch = filePath.match(/^(.+?):(\d+)(?:-(\d+))?$/);
          if (colonMatch) {
            const [, pathBeforeColon, startLine, endLine] = colonMatch;
            const isWindowsDrive = /^[A-Za-z]:$/.test(pathBeforeColon);

            if (!isWindowsDrive) {
              filePath = pathBeforeColon;
              lineRange = {
                start: parseInt(startLine, 10),
                end: endLine ? parseInt(endLine, 10) : undefined
              };
            }
          }
        }

        fileName = filePath.split('/').pop() || filePath;

        const isFolder = filePath.endsWith('/');
        if (!hrefValue.match(/^https?:\/\//) && !isFolder) {
          return (
            <button
              className="file-link"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleFileViewRequest(filePath, fileName, lineRange);
              }}
              type="button"
              style={{
                cursor: 'pointer',
                color: 'inherit',
                textDecoration: 'underline',
                background: 'none',
                border: 'none',
                font: 'inherit'
              }}
            >
              {children}
            </button>
          );
        }
      }
      
      if (typeof hrefValue === 'string' && hrefValue.startsWith('visualization:')) {
        const vizData = hrefValue.replace('visualization:', '');
        
        return (
          <button
            className="visualization-link"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              try {
                const visualization = JSON.parse(decodeURIComponent(vizData));
                handleOpenVisualization(visualization);
              } catch (error) {
                log.error('Failed to parse visualization data', { error });
              }
            }}
            type="button"
          >
            {children}
          </button>
        );
      }
      
      if (typeof hrefValue === 'string' && hrefValue.startsWith('tab:')) {
        const tabData = hrefValue.replace('tab:', '');
        
        return (
          <button
            className="tab-link"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              try {
                const tabInfo = JSON.parse(decodeURIComponent(tabData));
                handleTabOpen(tabInfo);
              } catch (error) {
                log.error('Failed to parse tab data', { error });
              }
            }}
            type="button"
            style={{ 
              cursor: 'pointer',
              color: '#3b82f6',
              textDecoration: 'underline',
              background: 'none',
              border: 'none',
              font: 'inherit'
            }}
          >
            {children}
          </button>
        );
      }
      
      if (typeof hrefValue === 'string' && (hrefValue.startsWith('http://') || hrefValue.startsWith('https://'))) {
        return (
          <a 
            href={hrefValue} 
            {...props}
            onClick={async (e) => {
              e.preventDefault();
              e.stopPropagation();
              try {
                await systemAPI.openExternal(hrefValue);
              } catch (error) {
                log.error('Failed to open external URL', { url: hrefValue, error });
              }
            }}
            style={{ cursor: 'pointer', color: '#3b82f6', textDecoration: 'underline' }}
          >
            {children}
          </a>
        );
      }
      
      return (
        <a 
          href={typeof hrefValue === 'string' ? hrefValue : undefined} 
          {...props}
          onClick={(e) => {
            e.preventDefault();
          }}
          style={{ cursor: 'pointer', color: 'inherit' }}
        >
          {children}
        </a>
      );
    },
    
    table({ children }: any) {
      return (
        <div className="table-wrapper">
          <table>{children}</table>
        </div>
      );
    },
    
    blockquote({ children }: any) {
      return <blockquote className="custom-blockquote">{children}</blockquote>;
    },
    
    ul({ children, ...props }: any) {
      return <ul {...props}>{children}</ul>;
    },
    
    ol({ children, ...props }: any) {
      return <ol {...props}>{children}</ol>;
    },
    
    li({ children, ...props }: any) {
      return <li {...props}>{children}</li>;
    },
    
    p({ children, ...props }: any) {
      return <p {...props}>{children}</p>;
    }
  }), [isStreaming, linkMap, handleFileViewRequest, handleOpenVisualization, handleTabOpen, parseLineRange, syntaxTheme, isLight]);
  
  return (
    <div className={`markdown-renderer ${className} ${isStreaming && contentStr ? 'markdown-renderer--streaming' : ''}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={components}
      >
        {markdownContent}
      </ReactMarkdown>
      
      {reproductionSteps && !isStreaming && (
        <ReproductionStepsBlock 
          steps={reproductionSteps}
          onProceed={onReproductionProceed}
        />
      )}
    </div>
  );
});

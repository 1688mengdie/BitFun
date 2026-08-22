/**
 * MermaidBlock component
 * Renders Mermaid diagrams in Markdown
 */

import React, { useEffect, useState, useRef, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { MermaidService } from '../../../tools/mermaid-editor/services/MermaidService';
import { mermaidAppearanceAdapter } from '@/infrastructure/appearance/adapters/MermaidAppearanceAdapter';
import { Loader2, AlertCircle, Code2, Copy, Check } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import './MermaidBlock.scss';

const log = createLogger('MermaidBlock');

/**
 * Sanitize Mermaid-rendered SVG markup before it is injected via
 * dangerouslySetInnerHTML.
 *
 * Mermaid is initialized with `securityLevel: 'loose'`, which lets label markup
 * such as `<img onerror=...>`, `<script>`, `on*` event attributes and
 * `javascript:` URLs survive rendering. We strip those while keeping the
 * structural SVG and text-container elements a rendered diagram needs.
 */
const ALLOWED_SVG_TAGS = new Set([
  // svg structure / shapes
  'svg', 'g', 'defs', 'style', 'marker', 'path', 'rect', 'circle', 'ellipse',
  'line', 'polyline', 'polygon', 'text', 'tspan', 'title', 'desc', 'use',
  'symbol', 'textpath', 'clippath', 'stop', 'pattern', 'mask', 'filter',
  'fegaussianblur', 'fecolormatrix', 'feoffset', 'feflood', 'feblend',
  'image', 'a',
  // gradient / pattern
  'lineargradient', 'radialgradient',
  // foreignObject content (mermaid htmlLabels) + text containers
  'foreignobject', 'div', 'span', 'p', 'br', 'b', 'strong', 'i', 'em', 'u',
  'small', 'sub', 'sup', 'ul', 'ol', 'li', 'pre', 'code', 'font',
  'table', 'thead', 'tbody', 'tfoot', 'tr', 'td', 'th', 'figure', 'h1', 'h2',
  'h3', 'h4', 'h5', 'h6',
]);

const ALLOWED_SVG_ATTRS = new Set([
  // core + namespace
  'id', 'class', 'xmlns', 'xmlns:xlink', 'xlink:href', 'xlink:title',
  'xml:space', 'role', 'aria-label', 'aria-roledescription', 'aria-hidden',
  'focusable', 'tabindex', 'title',
  // geometry / viewport
  'viewbox', 'preserveaspectratio', 'width', 'height', 'x', 'y', 'x1', 'y1',
  'x2', 'y2', 'cx', 'cy', 'r', 'rx', 'ry', 'd', 'points', 'pathlength',
  'transform', 'translate', 'scale', 'rotate', 'skewx', 'skewy', 'matrix',
  'offset', 'refx', 'refy', 'refwidth', 'refheight',
  // painting
  'fill', 'fill-opacity', 'fill-rule', 'stroke', 'stroke-width',
  'stroke-linecap', 'stroke-linejoin', 'stroke-miterlimit', 'stroke-dasharray',
  'stroke-dashoffset', 'stroke-opacity', 'opacity', 'color', 'flood-color',
  'flood-opacity', 'stop-color', 'stop-opacity',
  // typography / text
  'font-family', 'font-size', 'font-weight', 'font-style', 'font-variant',
  'text-anchor', 'dominant-baseline', 'baseline-shift', 'letter-spacing',
  'word-spacing', 'text-decoration', 'direction', 'unicode-bidi',
  'writing-mode', 'alignment-baseline', 'line-height', 'text-transform',
  'white-space', 'font-stretch',
  // markers / gradients / patterns / filters
  'marker-start', 'marker-mid', 'marker-end', 'marker', 'markerwidth',
  'markerheight', 'markerunits', 'orient', 'gradientunits', 'gradienttransform',
  'spreadmethod', 'patternunits', 'patterncontentunits', 'patterntransform',
  'maskunits', 'maskcontentunits', 'in', 'in2', 'result', 'stddeviation',
  'edgemode', 'kernelunitlength', 'tablevalues', 'values', 'type', 'mode',
  'interpolate', 'numoctaves', 'basefrequency', 'stitchtiles',
  // styles / display
  'style', 'display', 'visibility', 'overflow', 'clip', 'clip-path', 'clip-rule',
  'vector-effect', 'shape-rendering', 'text-rendering', 'image-rendering',
  'color-rendering', 'color-interpolation', 'color-interpolation-filters',
  'paint-order', 'mix-blend-mode', 'isolation', 'pointer-events', 'cursor',
  'filter', 'mask', 'enable-background',
  // animation
  'attributename', 'attributetype', 'begin', 'dur', 'end', 'repeatcount',
  'repeatdur', 'from', 'to', 'by', 'calcmode', 'additive', 'keytimes',
  'keysplines', 'keypoints', 'restart', 'fill-freeze',
  // html container attributes
  'align', 'valign', 'colspan', 'rowspan', 'cellpadding', 'cellspacing',
  'border', 'bgcolor', 'nowrap', 'dir', 'lang', 'col', 'row', 'span',
  'start', 'reversed', 'data-label',
]);

const DANGEROUS_TAGS = new Set([
  'script', 'iframe', 'object', 'embed', 'form', 'meta', 'link', 'base',
  'frame', 'frameset', 'noscript', 'template',
]);

const EVENT_ATTR_RE = /^on/i;
const URL_ATTRS = new Set(['href', 'src', 'xlink:href']);
const UNSAFE_URL_RE = /^\s*(?:javascript|vbscript|data:text\/html)\s*:/i;

export function sanitizeMermaidSvg(svgRaw: string): string {
  if (!svgRaw) return svgRaw;

  let doc: Document;
  try {
    doc = new DOMParser().parseFromString(svgRaw, 'text/html');
  } catch {
    return '';
  }

  const root = doc.body.firstElementChild;
  if (!root || root.localName.toLowerCase() !== 'svg') return '';

  const sanitizeNode = (node: Element) => {
    const tag = node.localName.toLowerCase();
    // Any element outside the allowed set (scripts, iframes, unknown HTML
    // injected via labels, etc.) is removed together with its subtree.
    if (DANGEROUS_TAGS.has(tag) || !ALLOWED_SVG_TAGS.has(tag)) {
      node.remove();
      return;
    }

    for (const attr of Array.from(node.attributes)) {
      const name = attr.name;
      const lowerName = name.toLowerCase();
      if (EVENT_ATTR_RE.test(lowerName)) {
        node.removeAttribute(name);
        continue;
      }
      if (URL_ATTRS.has(lowerName) && UNSAFE_URL_RE.test(attr.value)) {
        node.removeAttribute(name);
        continue;
      }
      if (!ALLOWED_SVG_ATTRS.has(lowerName)) {
        node.removeAttribute(name);
      }
    }

    Array.from(node.children).forEach((child) => sanitizeNode(child));
  };

  sanitizeNode(root);
  return root.outerHTML;
}

const svgCache = new Map<string, string>();

let appearanceRevision = 0;

const getCacheKey = (code: string): string => {
  return `${appearanceRevision}:${code.trim()}`;
};

const clearCache = () => {
  svgCache.clear();
  appearanceRevision = mermaidAppearanceAdapter.getRevision();
  log.debug('Cache cleared', { revision: appearanceRevision });
};

export interface MermaidBlockProps {
  code: string;
  isStreaming?: boolean;
  className?: string;
}

type RenderState = 'streaming' | 'incomplete' | 'loading' | 'rendered' | 'error';

const isCodeComplete = (code: string): boolean => {
  const trimmed = code.trim();
  if (!trimmed) return false;
  return /^(graph|flowchart|sequenceDiagram|classDiagram|stateDiagram|erDiagram|gantt|pie|journey|gitGraph|mindmap|timeline|quadrantChart)/m.test(trimmed);
};

export const MermaidBlock: React.FC<MermaidBlockProps> = ({
  code,
  isStreaming = false,
  className = ''
}) => {
  const { t } = useI18n('components');
  const cacheKey = getCacheKey(code.trim());
  const cachedSvg = svgCache.get(cacheKey);
  
  const [state, setState] = useState<RenderState>(() => {
    if (cachedSvg) return 'rendered';
    if (isStreaming) return 'streaming';
    if (!code.trim() || !isCodeComplete(code)) return 'incomplete';
    return 'loading';
  });
  const [svgContent, setSvgContent] = useState<string>(cachedSvg || '');
  const [error, setError] = useState<string>('');
  const [showCode, setShowCode] = useState(false);
  const [copied, setCopied] = useState(false);
  
  const [currentAppearanceRevision, setCurrentAppearanceRevision] = useState(
    mermaidAppearanceAdapter.getRevision(),
  );
  
  const mermaidService = useRef(MermaidService.getInstance());
  const renderTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const currentCodeRef = useRef<string>('');

  const renderDiagram = useCallback(async (codeToRender: string) => {
    const trimmedCode = codeToRender.trim();
    const key = getCacheKey(trimmedCode);
    
    if (!isCodeComplete(trimmedCode)) {
      setState('incomplete');
      return;
    }

    const cached = svgCache.get(key);
    if (cached) {
      setSvgContent(cached);
      setState('rendered');
      return;
    }

    setState('loading');
    setError('');

    try {
      const svg = await mermaidService.current.renderDiagram(trimmedCode);
      if (currentCodeRef.current === trimmedCode) {
        svgCache.set(key, svg);
        setSvgContent(svg);
        setState('rendered');
      }
    } catch (err) {
      if (currentCodeRef.current === trimmedCode) {
        setError(err instanceof Error ? err.message : t('mermaidBlock.renderFailed'));
        setState('error');
      }
    }
  }, [t]);

  useEffect(() => {
    const trimmedCode = code.trim();
    currentCodeRef.current = trimmedCode;

    if (renderTimeoutRef.current) {
      clearTimeout(renderTimeoutRef.current);
      renderTimeoutRef.current = null;
    }

    if (isStreaming) {
      setState('streaming');
      return;
    }

    if (!trimmedCode || !isCodeComplete(trimmedCode)) {
      setState('incomplete');
      return;
    }

    const key = getCacheKey(trimmedCode);
    const cached = svgCache.get(key);
    if (cached) {
      setSvgContent(cached);
      setState('rendered');
      return;
    }

    renderTimeoutRef.current = setTimeout(() => {
      renderDiagram(trimmedCode);
    }, 200);

    return () => {
      if (renderTimeoutRef.current) {
        clearTimeout(renderTimeoutRef.current);
      }
    };
  }, [code, isStreaming, renderDiagram, currentAppearanceRevision]);

  useEffect(() => {
    return mermaidAppearanceAdapter.subscribe(() => {
      clearCache();
      setCurrentAppearanceRevision(mermaidAppearanceAdapter.getRevision());
      setSvgContent('');
      setState('loading');
    });
  }, []);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      log.error('Failed to copy code', err);
    }
  }, [code]);

  const renderContent = () => {
    switch (state) {
      case 'streaming':
        return (
          <div data-bf-component="mermaid-block" data-bf-part="streaming" className="mermaid-block__streaming">
            <div data-bf-component="mermaid-block" data-bf-part="codePreview" className="mermaid-block__code-preview">
              <pre data-bf-component="mermaid-block" data-bf-part="code" className="mermaid-code">
                <code>{code}</code>
                <span className="streaming-cursor">█</span>
              </pre>
            </div>
          </div>
        );

      case 'incomplete':
        return (
          <div className="mermaid-block__incomplete">
            <div data-bf-component="mermaid-block" data-bf-part="codePreview" className="mermaid-block__code-preview">
              <pre data-bf-component="mermaid-block" data-bf-part="code" className="mermaid-code">
                <code>{code}</code>
              </pre>
            </div>
            <div data-bf-component="mermaid-block" data-bf-part="hint" className="mermaid-block__hint">
              <AlertCircle size={14} />
              <span>{t('mermaidBlock.codeIncomplete')}</span>
            </div>
          </div>
        );

      case 'loading':
        return (
          <div data-bf-component="mermaid-block" data-bf-part="loading" className="mermaid-block__loading">
            <div className="mermaid-block__loading-indicator">
              <Loader2 size={20} className="spinning" />
              <span>{t('mermaidBlock.rendering')}</span>
            </div>
          </div>
        );

      case 'rendered':
        return (
          <div data-bf-component="mermaid-block" data-bf-part="rendered" className="mermaid-block__rendered">
            <div 
              className="mermaid-block__diagram"
              data-bf-component="mermaid-block"
              data-bf-part="diagram"
              dangerouslySetInnerHTML={{ __html: sanitizeMermaidSvg(svgContent) }}
            />
            
            <div data-bf-component="mermaid-block" data-bf-part="actions" className="mermaid-block__actions">
              <button
                data-bf-component="mermaid-block"
                data-bf-part="action"
                className="mermaid-icon-btn"
                onClick={() => setShowCode(!showCode)}
                title={showCode ? t('mermaidBlock.hideCode') : t('mermaidBlock.showCode')}
              >
                <Code2 size={14} />
              </button>
              <button
                data-bf-component="mermaid-block"
                data-bf-part="action"
                data-bf-state={copied ? 'copied' : undefined}
                className={`mermaid-icon-btn ${copied ? 'copied' : ''}`}
                onClick={handleCopy}
                title={t('mermaidBlock.copyCode')}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
            </div>

            {showCode && (
              <div data-bf-component="mermaid-block" data-bf-part="source" className="mermaid-block__source">
                <pre data-bf-component="mermaid-block" data-bf-part="code" className="mermaid-code">
                  <code>{code}</code>
                </pre>
              </div>
            )}
          </div>
        );

      case 'error':
        return (
          <div data-bf-component="mermaid-block" data-bf-part="error" className="mermaid-block__error">
            <div className="mermaid-block__error-message">
              <AlertCircle size={16} />
              <span>{t('mermaidBlock.renderFailed')}: {error}</span>
            </div>
            <div data-bf-component="mermaid-block" data-bf-part="codePreview" className="mermaid-block__code-preview">
              <pre data-bf-component="mermaid-block" data-bf-part="code" className="mermaid-code">
                <code>{code}</code>
              </pre>
            </div>
            <div data-bf-component="mermaid-block" data-bf-part="actions" className="mermaid-block__actions">
              <button
                data-bf-component="mermaid-block"
                data-bf-part="action"
                data-bf-state={copied ? 'copied' : undefined}
                className={`mermaid-icon-btn ${copied ? 'copied' : ''}`}
                onClick={handleCopy}
                title={t('mermaidBlock.copyCode')}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
            </div>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className={`mermaid-block mermaid-block--${state} ${className}`} data-bf-component="mermaid-block" data-bf-part="root" data-bf-state={state === 'error' ? 'error' : state === 'streaming' ? 'streaming' : state === 'loading' ? 'loading' : undefined}>
      {renderContent()}
    </div>
  );
};

export default MermaidBlock;

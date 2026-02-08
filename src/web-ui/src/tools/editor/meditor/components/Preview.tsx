import React, { useEffect, useRef } from 'react'
import { createLogger } from '@/shared/utils/logger'
import { useMarkdown } from '../hooks/useMarkdown'
import { MermaidService } from '@/tools/mermaid-editor/services/MermaidService'
import { loadLocalImages } from '../utils/loadLocalImages'
import { useI18n } from '@/infrastructure/i18n'
import type { RenderOptions } from '../types'
import './Preview.scss'

const log = createLogger('Preview')

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

interface PreviewProps {
  value: string
  options?: RenderOptions
  /**
   * Directory path of the Markdown file.
   * Used to resolve relative image paths.
   */
  basePath?: string
}

export const Preview: React.FC<PreviewProps> = ({ value, options, basePath }) => {
  const { t } = useI18n('tools')
  const mergedOptions = React.useMemo(() => ({
    ...options,
    basePath
  }), [options, basePath])
  
  const { html, isLoading } = useMarkdown(value, mergedOptions)
  const previewRef = useRef<HTMLDivElement>(null)
  const mermaidService = useRef(MermaidService.getInstance())
  const lastHtmlRef = useRef<string>('')

  useEffect(() => {
    if (previewRef.current && html) {
      if (lastHtmlRef.current !== html) {
        previewRef.current.innerHTML = html
        lastHtmlRef.current = html
      }
    }
  }, [html])

  useEffect(() => {
    if (previewRef.current && html) {
      const mermaidContainers = previewRef.current.querySelectorAll('.mermaid-container:not(.mermaid-rendered)')
      if (mermaidContainers.length === 0) {
        return
      }
      
      let isCancelled = false
      
      const renderMermaidDiagrams = async () => {
        for (const container of Array.from(mermaidContainers)) {
          if (isCancelled) break
          
          const mermaidCode = container.getAttribute('data-mermaid-code')
          
          if (mermaidCode) {
            try {
              const svg = await mermaidService.current.renderDiagram(mermaidCode)
              if (!isCancelled && previewRef.current?.contains(container)) {
                container.innerHTML = svg
                container.classList.add('mermaid-rendered')
              }
            } catch (error) {
              log.error('Mermaid render failed', error)
              if (!isCancelled && previewRef.current?.contains(container)) {
                const rawMsg = error instanceof Error ? error.message : t('editor.meditor.unknownError')
                const detail = rawMsg.replace(/^Render failed:\s*/i, '')
                const title = escapeHtml(t('editor.meditor.mermaidRenderFailed'))
                container.innerHTML = `<div class="mermaid-error"><div class="mermaid-error-title">${title}</div><hr class="mermaid-error-divider"/><div class="mermaid-error-detail">${escapeHtml(detail)}</div></div>`
              }
            }
          }
        }
      }

      const loadImages = async () => {
        if (previewRef.current && !isCancelled) {
          await loadLocalImages(previewRef.current)
        }
      }

      renderMermaidDiagrams()
      loadImages()
      
      return () => {
        isCancelled = true
      }
    }
  }, [html])

  return (
    <div className="m-editor-preview">
      {isLoading && (
        <div className="m-editor-preview-loading">
          {t('editor.meditor.loading')}
        </div>
      )}
      {/* Use ref to manually control innerHTML, preventing React re-render from overwriting Mermaid SVG */}
      <div
        ref={previewRef}
        className="m-editor-preview-content markdown-body"
      />
    </div>
  )
}


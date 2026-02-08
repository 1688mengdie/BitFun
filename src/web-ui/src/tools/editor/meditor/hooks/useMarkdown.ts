import { useState, useEffect, useMemo, useRef } from 'react'
import { MarkdownParser } from '../utils/markdown'
import type { RenderOptions } from '../types'
import { createLogger } from '@/shared/utils/logger'
import { useI18n } from '@/infrastructure/i18n'

const log = createLogger('useMarkdown')

/**
 * Markdown rendering hook (simplified).
 */
export function useMarkdown(
  markdown: string,
  options: RenderOptions = {}
) {
  const { t } = useI18n('tools')
  const [html, setHtml] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  
  const basePath = options.basePath
  const parser = useMemo(() => new MarkdownParser({ ...options, basePath }), [basePath])
  
  const lastMarkdownRef = useRef<string>('')

  useEffect(() => {
    if (markdown === lastMarkdownRef.current) {
      return
    }
    
    lastMarkdownRef.current = markdown
    let cancelled = false
    
    const parse = async () => {
      setIsLoading(true)
      try {
        const result = await parser.parse(markdown)
        if (!cancelled) {
          setHtml(result)
        }
      } catch (error) {
        log.error('Markdown parsing failed', error)
        if (!cancelled) {
          setHtml(`<p>${t('editor.meditor.parseError')}</p>`)
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false)
        }
      }
    }
    
    parse()
    
    return () => {
      cancelled = true
    }
  }, [markdown, parser, t])

  return { html, isLoading, parser }
}


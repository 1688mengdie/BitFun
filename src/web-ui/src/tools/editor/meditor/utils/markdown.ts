import { unified } from 'unified'
import remarkParse from 'remark-parse'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import remarkRehype from 'remark-rehype'
import rehypeKatex from 'rehype-katex'
import rehypeStringify from 'rehype-stringify'
import rehypeHighlight from 'rehype-highlight'
import { rehypeMermaid } from './rehype-mermaid'
import { rehypeLocalImages } from './rehype-local-images'
import { createLogger } from '@/shared/utils/logger'
import type { RenderOptions } from '../types'

/**
 * Markdown parser
 */
export class MarkdownParser {
  private options: RenderOptions

  constructor(options: RenderOptions = {}) {
    this.options = {
      math: true,
      highlight: true,
      emoji: true,
      taskList: true,
      table: true,
      linkify: true,
      ...options
    }
  }

  /**
   * Convert Markdown to HTML
   */
  async parse(markdown: string): Promise<string> {
    const processor = unified()
      .use(remarkParse)
      .use(remarkGfm)
      .use(this.options.math ? remarkMath : () => {})
      .use(remarkRehype, { allowDangerousHtml: true })
      .use(this.options.math ? rehypeKatex : () => {})
      .use(rehypeMermaid)
      .use(rehypeLocalImages, { basePath: this.options.basePath })
      .use(this.options.highlight ? rehypeHighlight : () => {})
      .use(rehypeStringify, { allowDangerousHtml: true })

    const result = await processor.process(markdown)
    return String(result)
  }

  /**
   * Synchronous parse (used for live preview)
   */
  parseSync(markdown: string): string {
    try {
      const processor = unified()
        .use(remarkParse)
        .use(remarkGfm)
        .use(this.options.math ? remarkMath : () => {})
        .use(remarkRehype, { allowDangerousHtml: true })
        .use(this.options.math ? rehypeKatex : () => {})
        .use(rehypeMermaid)
        .use(rehypeLocalImages, { basePath: this.options.basePath })
        .use(this.options.highlight ? rehypeHighlight : () => {})
        .use(rehypeStringify, { allowDangerousHtml: true })

      const result = processor.processSync(markdown)
      return String(result)
    } catch (error) {
      createLogger('MarkdownParser').error('Sync parse failed', error);
      return markdown
    }
  }

  /**
   * Update options
   */
  updateOptions(options: Partial<RenderOptions>) {
    this.options = { ...this.options, ...options }
  }
}

/**
 * Debounce helper
 */
export function debounce<T extends (...args: any[]) => any>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let timeout: ReturnType<typeof setTimeout> | null = null

  return function executedFunction(...args: Parameters<T>) {
    const later = () => {
      timeout = null
      func(...args)
    }

    if (timeout) {
      clearTimeout(timeout)
    }
    timeout = setTimeout(later, wait)
  }
}

/**
 * Throttle helper
 */
export function throttle<T extends (...args: any[]) => any>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let timeout: ReturnType<typeof setTimeout> | null = null
  let previous = 0

  return function executedFunction(...args: Parameters<T>) {
    const now = Date.now()
    const remaining = wait - (now - previous)

    if (remaining <= 0 || remaining > wait) {
      if (timeout) {
        clearTimeout(timeout)
        timeout = null
      }
      previous = now
      func(...args)
    } else if (!timeout) {
      timeout = setTimeout(() => {
        previous = Date.now()
        timeout = null
        func(...args)
      }, remaining)
    }
  }
}


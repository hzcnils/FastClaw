import DOMPurify from 'dompurify'

/**
 * 净化不可信 HTML（RAG/chunk/文档内容），防存储型 XSS。
 * 保留 KaTeX 所需的 MathML/SVG。
 */
export function sanitizeHtml(html: string): string {
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true, mathMl: true, svg: true },
    ADD_ATTR: ['loading', 'target'],
  })
}

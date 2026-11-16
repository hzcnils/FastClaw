// FastClaw · 轻量网页「阅读模式」正文提取 + 净化
//
// fastclaw 未引入 @mozilla/readability，此处用 DOMParser 做降级正文抽取：
//   1. 移除脚本/样式/导航等非正文节点
//   2. 优先取 <article>/<main>/[role=main]，退化到 <body>
//   3. 相对链接转绝对（src/href/srcset）
//   4. 剥离 on* 事件属性与 javascript: 链接
//   5. DOMPurify 兜底净化（防不可信外部 HTML）
// 最终产物为一份自带阅读样式的完整 HTML 文档，交给 sandbox iframe srcdoc。
import DOMPurify from 'dompurify'

const REMOVE_SELECTOR = [
  'script', 'style', 'noscript', 'iframe', 'object', 'embed', 'link', 'meta',
  'base', 'template', 'form', 'nav', 'footer', 'aside', 'header', 'button',
  'input', 'select', 'textarea', 'svg', 'canvas',
].join(', ')

const READER_CSS = `
  * { box-sizing: border-box; }
  html, body { margin: 0; padding: 0; }
  body {
    margin: 0 auto;
    padding: 24px 20px 56px;
    max-width: 760px;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
      'Helvetica Neue', Arial, 'PingFang SC', 'Microsoft YaHei', sans-serif;
    font-size: 16px;
    line-height: 1.75;
    color: #1f2328;
    background: #ffffff;
    word-break: break-word;
  }
  img { max-width: 100%; height: auto; }
  video { max-width: 100%; }
  pre { overflow-x: auto; background: #f6f8fa; padding: 12px; border-radius: 6px; font-size: 13px; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    background: #f6f8fa; padding: 1px 5px; border-radius: 3px; font-size: 0.9em; }
  pre code { background: none; padding: 0; }
  a { color: #0969da; }
  blockquote { margin: 0; padding: 0 1em; color: #57606a; border-left: 3px solid #d0d7de; }
  table { border-collapse: collapse; width: 100%; margin: 12px 0; }
  th, td { border: 1px solid #d0d7de; padding: 6px 13px; }
  h1, h2, h3, h4, h5, h6 { margin-top: 24px; margin-bottom: 16px; line-height: 1.25; }
  p { margin: 0 0 12px; }
  li { margin: 4px 0; }
`

/** 把相对链接转绝对（src / href / srcset） */
function absolutize(root: Element, base: string): void {
  let baseUrl: URL | null = null
  try { baseUrl = new URL(base) } catch { /* base 非法则放弃转绝对 */ }
  if (!baseUrl) return

  root.querySelectorAll('[src]').forEach((el) => {
    const v = el.getAttribute('src')
    if (v) { try { el.setAttribute('src', new URL(v, baseUrl).href) } catch { /* ignore */ } }
  })
  root.querySelectorAll('[href]').forEach((el) => {
    const v = el.getAttribute('href')
    if (v) { try { el.setAttribute('href', new URL(v, baseUrl).href) } catch { /* ignore */ } }
  })
  root.querySelectorAll('[srcset]').forEach((el) => {
    const v = el.getAttribute('srcset')
    if (!v) return
    const fixed = v.split(',').map((part) => {
      const seg = part.trim().split(/\s+/)
      if (!seg.length || !seg[0]) return part
      try { return [new URL(seg[0], baseUrl).href, ...seg.slice(1)].join(' ') } catch { return part }
    }).join(', ')
    el.setAttribute('srcset', fixed)
  })
}

/** 剥离事件属性与 javascript: 链接 */
function stripUnsafe(root: Element): void {
  root.querySelectorAll('*').forEach((el) => {
    for (const attr of Array.from(el.attributes)) {
      if (attr.name.toLowerCase().startsWith('on')) el.removeAttribute(attr.name)
    }
    const href = el.getAttribute('href')
    if (href && href.trim().toLowerCase().startsWith('javascript:')) el.removeAttribute('href')
    const src = el.getAttribute('src')
    if (src && src.trim().toLowerCase().startsWith('javascript:')) el.removeAttribute('src')
  })
}

/**
 * 提取正文并生成阅读模式的完整 HTML 文档。
 * 提取失败（无正文 / 解析异常）返回空字符串，调用方据此禁用「阅读」模式。
 */
export function buildReaderHtml(html: string, base: string): string {
  if (!html) return ''
  try {
    const doc = new DOMParser().parseFromString(html, 'text/html')
    doc.querySelectorAll(REMOVE_SELECTOR).forEach((el) => el.remove())
    const root = doc.querySelector('article')
      || doc.querySelector('main')
      || doc.querySelector('[role="main"]')
      || doc.body
    if (!root) return ''
    absolutize(root, base)
    stripUnsafe(root)
    const content = root.innerHTML
    if (!content || !content.trim()) return ''
    const clean = DOMPurify.sanitize(content, { ADD_ATTR: ['target', 'rel'] })
    if (!clean) return ''
    return `<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><style>${READER_CSS}</style></head><body>${clean}</body></html>`
  } catch {
    return ''
  }
}

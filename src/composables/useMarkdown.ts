// FastClaw · Markdown 渲染 (markdown-it + KaTeX + highlight.js + mermaid)
import MarkdownIt from 'markdown-it'
import hljs from 'highlight.js'
import katex from 'katex'

// --- 预处理：修复 LLM 输出中常见的格式问题 ---
function preprocessContent(text: string): string {
  // 把独占一行的 $ ... $ 转成 $$ ... $$
  const lines = text.split('\n')
  const out: string[] = []
  let i = 0
  while (i < lines.length) {
    if (lines[i].trim() === '$') {
      const buf: string[] = []
      let j = i + 1
      let closed = false
      while (j < lines.length) {
        if (lines[j].trim() === '$') { closed = true; break }
        buf.push(lines[j])
        j++
      }
      if (closed && buf.length > 0) {
        out.push('$$')
        out.push(...buf)
        out.push('$$')
        i = j + 1
        continue
      }
    }
    out.push(lines[i])
    i++
  }
  let result = out.join('\n')

  // 确保 $$ 前后有空行
  result = result.replace(/([^\n])\n(\$\$)/g, '$1\n\n$2')
  result = result.replace(/(\$\$)\n([^\n$])/g, '$1\n\n$2')

  return result
}

// --- KaTeX 插件（修复版） ---
function katexPlugin(md: MarkdownIt) {

  // ===== 行内公式 $...$ =====
  function inlineRule(state: any, silent: boolean) {
    const src = state.src
    const pos = state.pos
    const max = state.posMax

    // 必须以 $ 开头
    if (src.charCodeAt(pos) !== 0x24) return false

    // 不能是 $$（那是块级）
    if (pos + 1 <= max && src.charCodeAt(pos + 1) === 0x24) return false

    // 开头 $ 后面不能是空格或换行（避免误匹配货币符号）
    if (pos + 1 > max) return false
    const afterDollar = src.charCodeAt(pos + 1)
    if (afterDollar === 0x20 || afterDollar === 0x0A || afterDollar === 0x0D) return false

    // 向后寻找配对的 $
    let end = pos + 1
    while (end <= max) {
      const ch = src.charCodeAt(end)
      if (ch === 0x24) {
        // 检查不是转义的 \$
        if (end > 0 && src.charCodeAt(end - 1) === 0x5C) {
          end++
          continue
        }
        // 配对 $ 前面不能是空格（避免 "10$ 20$" 这种误匹配）
        if (end > pos + 1 && src.charCodeAt(end - 1) === 0x20) {
          end++
          continue
        }
        // 找到了配对的 $
        break
      }
      // 不能跨越换行
      if (ch === 0x0A) return false
      end++
    }

    if (end > max) return false

    const content = src.slice(pos + 1, end)
    if (!content.trim()) return false

    if (!silent) {
      const token = state.push('katex_inline', 'math', 0)
      token.content = content
      token.markup = '$'
    }

    state.pos = end + 1
    return true
  }

  // ===== 块级公式 $$...$$ =====
  function blockRule(state: any, startLine: number, endLine: number, silent: boolean) {
    const startPos = state.bMarks[startLine] + state.tShift[startLine]
    const maxPos = state.eMarks[startLine]

    if (startPos + 2 > maxPos) return false
    if (state.src.charCodeAt(startPos) !== 0x24 || state.src.charCodeAt(startPos + 1) !== 0x24) return false
    if (silent) return true

    // 检查 $$ 后面同一行是否有内容（单行块级公式 $$ ... $$）
    const afterOpen = state.src.slice(startPos + 2, maxPos).trim()
    if (afterOpen && afterOpen.endsWith('$$')) {
      // 单行块级公式: $$ content $$
      const content = afterOpen.slice(0, -2).trim()
      if (content) {
        const token = state.push('katex_block', 'math', 0)
        token.content = content
        token.markup = '$$'
        token.block = true
        token.map = [startLine, startLine + 1]
        state.line = startLine + 1
        return true
      }
    }

    // 多行块级公式
    let nextLine = startLine
    let hasEnding = false
    while (true) {
      nextLine++
      if (nextLine >= endLine) break
      const ls = state.bMarks[nextLine] + state.tShift[nextLine]
      const lm = state.eMarks[nextLine]
      if (ls < lm && state.src.charCodeAt(ls) === 0x24 && state.src.charCodeAt(ls + 1) === 0x24) {
        hasEnding = true
        break
      }
    }

    const content = state.getLines(startLine + 1, nextLine, state.tShift[startLine], false).trim()
    const token = state.push('katex_block', 'math', 0)
    token.content = content
    token.markup = '$$'
    token.block = true
    token.map = [startLine, hasEnding ? nextLine + 1 : nextLine]
    state.line = hasEnding ? nextLine + 1 : nextLine
    return true
  }

  md.inline.ruler.after('escape', 'katex_inline', inlineRule)
  md.block.ruler.before('fence', 'katex_block', blockRule, {
    alt: ['paragraph', 'reference', 'blockquote', 'list'],
  })

  // ===== 渲染器 =====
  md.renderer.rules.katex_inline = (tokens: any[], idx: number) => {
    try {
      return katex.renderToString(tokens[idx].content, {
        throwOnError: false,
        displayMode: false,
      })
    } catch {
      return `<code class="katex-error">${md.utils.escapeHtml(tokens[idx].content)}</code>`
    }
  }

  md.renderer.rules.katex_block = (tokens: any[], idx: number) => {
    try {
      return `<div class="katex-block">${katex.renderToString(tokens[idx].content, {
        throwOnError: false,
        displayMode: true,
      })}</div>`
    } catch {
      return `<pre class="katex-error">${md.utils.escapeHtml(tokens[idx].content)}</pre>`
    }
  }
}

// --- 创建 md 实例 ---
const md: MarkdownIt = new MarkdownIt({
  html: true,
  linkify: true,
  typographer: true,
  highlight(str: string, lang: string): string {
    if (lang === 'mermaid') {
      return `<div class="mermaid-block"><pre class="mermaid">${md.utils.escapeHtml(str)}</pre></div>`
    }
    if (lang && hljs.getLanguage(lang)) {
      try {
        const result = hljs.highlight(str, { language: lang, ignoreIllegals: true })
        return `<pre class="hljs code-block" data-lang="${lang}"><div class="code-header"><span class="code-lang">${lang}</span><button class="code-copy-btn" onclick="navigator.clipboard.writeText(this.closest('.code-block').querySelector('code').textContent).then(()=>{this.textContent='已复制';setTimeout(()=>this.textContent='复制',1500)})">复制</button></div><code class="hljs language-${lang}">${result.value}</code></pre>`
      } catch { /* fallthrough */ }
    }
    try {
      const result = hljs.highlightAuto(str)
      return `<pre class="hljs code-block"><div class="code-header"><span class="code-lang">code</span><button class="code-copy-btn" onclick="navigator.clipboard.writeText(this.closest('.code-block').querySelector('code').textContent).then(()=>{this.textContent='已复制';setTimeout(()=>this.textContent='复制',1500)})">复制</button></div><code class="hljs">${result.value}</code></pre>`
    } catch { /* fallthrough */ }
    return `<pre class="hljs code-block"><code>${md.utils.escapeHtml(str)}</code></pre>`
  },
})

katexPlugin(md)

// 表格包裹层
const defaultTableOpen = md.renderer.rules.table_open || function(_t: any, idx: any, opts: any, _e: any, self: any) { return self.renderToken(_t, idx, opts) }
md.renderer.rules.table_open = function(tokens: any, idx: any, opts: any, env: any, self: any) {
  return '<div class="md-table-wrapper">' + defaultTableOpen(tokens, idx, opts, env, self)
}
const defaultTableClose = md.renderer.rules.table_close || function(_t: any, idx: any, opts: any, _e: any, self: any) { return self.renderToken(_t, idx, opts) }
md.renderer.rules.table_close = function(tokens: any, idx: any, opts: any, env: any, self: any) {
  return defaultTableClose(tokens, idx, opts, env, self) + '</div>'
}

export function useMarkdown() {
  const render = (text: string): string => {
    if (!text) return ''
    return md.render(preprocessContent(text))
  }
  const renderStreaming = (text: string): string => {
    if (!text) return ''
    try { return md.render(preprocessContent(text)) } catch { return `<p>${text}</p>` }
  }
  return { render, renderStreaming }
}
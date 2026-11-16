import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile, writeFile } from '@tauri-apps/plugin-fs'
import { assertExportCurrent, type ExportControl } from './exportControl'
import {
  Document,
  Packer,
  Paragraph,
  TextRun,
  ImageRun,
  HeadingLevel,
  Table,
  TableRow,
  TableCell,
  WidthType,
  BorderStyle,
  AlignmentType,
  type IParagraphOptions,
} from 'docx'
function timestamp(): string {
  const d = new Date()
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`
}

// ── Markdown export ────────────────────────────────────────────────

export async function exportMarkdown(content: string, control?: ExportControl): Promise<boolean> {
  assertExportCurrent(control)
  const filePath = await save({
    title: '导出 Markdown',
    defaultPath: `fastclaw-answer_${timestamp()}.md`,
    filters: [{ name: 'Markdown', extensions: ['md'] }],
  })
  if (!filePath) return false
  assertExportCurrent(control)
  await writeTextFile(filePath, content)
  return true
}

// ── PDF export (后端 jmisformu 生成，避免前端卡顿) ──────────────────

/** 将 markdown 渲染为 HTML 的函数签名（方案 B 中已不再使用，保留以兼容调用方） */
export type RenderHtmlFn = (markdown: string) => string

/** PDF 导出结果 */
export interface ExportPdfResult {
  success: boolean
  canceled?: boolean
  filePath?: string
  error?: string
}

/**
 * 将 markdown 内容导出为 PDF。
 * 方案 B：PDF 生成全部放到 Rust 后端 jmisformu 完成（支持中文与矢量公式），
 * 前端仅负责弹出保存对话框并写入文件，彻底解决前端 html2canvas 渲染导致的卡顿与样式污染。
 * @param content  原始 markdown 文本
 * @param _renderHtml  已废弃，仅用于保持现有调用签名
 * @param _onProgress  已废弃，仅用于保持现有调用签名
 */
export interface ExportProgress {
  stage: 'render' | 'slice'
  current: number
  total: number
}

export async function exportPdf(
  content: string,
  _renderHtml: RenderHtmlFn,
  _onProgress?: (progress: ExportProgress) => void,
  control?: ExportControl,
): Promise<ExportPdfResult> {
  // 方案 B：PDF 生成全部放到 Rust 后端（jmisformu），前端只做保存对话框与文件写入，
  // 从而彻底避免 html2canvas + jsPDF 在主线程渲染导致的界面卡顿与样式污染。
  assertExportCurrent(control)
  const invoke = await getInvoke()
  assertExportCurrent(control)
  if (!invoke) {
    return { success: false, error: 'Tauri 环境不可用' }
  }

  try {
    const bytes: number[] = await invoke<number[]>('export_pdf', { markdown: content })
    assertExportCurrent(control)

    const filePath = await save({
      title: '导出 PDF',
      defaultPath: `fastclaw-answer_${timestamp()}.pdf`,
      filters: [{ name: 'PDF Document', extensions: ['pdf'] }],
    })
    if (!filePath) {
      return { success: false, canceled: true }
    }

    assertExportCurrent(control)
    await writeFile(filePath, new Uint8Array(bytes))
    return { success: true, filePath }
  } catch (err: any) {
    if (err?.name === 'AbortError') return { success: false, canceled: true }
    return { success: false, error: err?.message || String(err) }
  }
}

// ── Word (docx) export ─────────────────────────────────────────────

interface InlineStyle {
  bold: boolean
  italic: boolean
  underline: boolean
  strike: boolean
  font: string
  color: string
  highlight: string
}

const DEFAULT_STYLE: InlineStyle = {
  bold: false,
  italic: false,
  underline: false,
  strike: false,
  font: '',
  color: '',
  highlight: '',
}

function cloneStyle(s: InlineStyle): InlineStyle {
  return { ...s }
}

function styleToTextRunOptions(s: InlineStyle): Partial<TextRun> {
  const opts: Partial<TextRun> = {}
  if (s.bold) opts.bold = true
  if (s.italic) opts.italics = true
  if (s.underline) opts.underline = { type: 'single' as const }
  if (s.strike) opts.strike = true
  if (s.font) opts.font = { name: s.font }
  if (s.color) opts.color = s.color
  if (s.highlight) opts.highlight = s.highlight
  return opts
}

/**
 * 将 HTML 中的 KaTeX 公式渲染为 PNG 图片并替换为 <img> 标签，
 * 使后续 parseHtmlToDocx 可将公式作为 ImageRun 嵌入 Word。
 */
async function preprocessFormulas(html: string, control?: ExportControl): Promise<string> {
  assertExportCurrent(control)
  const container = document.createElement('div')
  container.innerHTML = html
  container.style.position = 'absolute'
  container.style.left = '-9999px'
  document.body.appendChild(container)
  try {
    const katexEls = container.querySelectorAll('.katex')
    const replacements: Array<{ old: Element; img: HTMLImageElement }> = []

    for (const el of katexEls) {
      assertExportCurrent(control)
      const svgEl = el.querySelector('svg')
      if (!svgEl) {
        // KaTeX 可能渲染为 HTML 模式（无 SVG），跳过
        // 尝试直接用整个元素截图
        const rect = el.getBoundingClientRect()
        const w = Math.max(rect.width, 30)
        const h = Math.max(rect.height, 16)
        const png = await elementToPng(el, w, h)
        assertExportCurrent(control)
        if (!png) continue
        const img = document.createElement('img')
        img.src = png
        img.width = Math.round(w * 2)
        img.height = Math.round(h * 2)
        img.alt = el.textContent?.trim() || 'formula'
        img.setAttribute('data-katex', 'true')
        replacements.push({ old: el, img })
        continue
      }

      const svgString = new XMLSerializer().serializeToString(svgEl)
      // 获取实际尺寸
      const viewBox = svgEl.getAttribute('viewBox')
      let svgW = svgEl.clientWidth || 100
      let svgH = svgEl.clientHeight || 30
      if (viewBox) {
        const parts = viewBox.split(/\s+/)
        if (parts.length >= 3) svgW = parseFloat(parts[2])
        if (parts.length >= 4) svgH = parseFloat(parts[3])
      }

      const png = await svgToPng(svgString, svgW, svgH)
      assertExportCurrent(control)
      if (!png) continue

      const img = document.createElement('img')
      img.src = png
      img.width = Math.round(svgW * 2)
      img.height = Math.round(svgH * 2)
      img.alt = el.textContent?.trim() || 'formula'
      img.setAttribute('data-katex', 'true')
      replacements.push({ old: el, img })
    }

    for (const { old, img } of replacements) {
      old.parentNode?.replaceChild(img, old)
    }

    return container.innerHTML
  } finally { container.remove() }
}

/** 将 SVG 字符串渲染为 PNG data URL */
function svgToPng(svgString: string, w: number, h: number): Promise<string> {
  return new Promise((resolve) => {
    const svgBlob = new Blob([svgString], { type: 'image/svg+xml;charset=utf-8' })
    const url = URL.createObjectURL(svgBlob)
    const img = new Image()
    img.onload = () => {
      let png = ''
      try {
        const canvas = document.createElement('canvas')
        const scale = 2
        canvas.width = Math.ceil(w * scale)
        canvas.height = Math.ceil(h * scale)
        const ctx = canvas.getContext('2d')
        if (ctx) {
          ctx.fillStyle = '#ffffff'
          ctx.fillRect(0, 0, canvas.width, canvas.height)
          ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
          png = canvas.toDataURL('image/png')
        }
      } catch { /* A failed formula rasterization must not strand the export promise. */ }
      finally { URL.revokeObjectURL(url); resolve(png) }
    }
    img.onerror = () => {
      URL.revokeObjectURL(url)
      resolve('')
    }
    img.src = url
  })
}

/** 将 HTML 元素渲染为 PNG data URL（用于非 SVG KaTeX 渲染） */
function elementToPng(el: Element, w: number, h: number): Promise<string> {
  return import('html2canvas').then(async ({ default: h2c }) => {
    try {
      const canvas = await h2c(el as HTMLElement, {
        scale: 2,
        backgroundColor: '#ffffff',
        logging: false,
      })
      return canvas.toDataURL('image/png')
    } catch {
      return ''
    }
  })
}

/**
 * Convert rendered HTML to docx Paragraph / Table elements.
 */
function parseHtmlToDocx(html: string): (Paragraph | Table)[] {
  const container = document.createElement('div')
  container.innerHTML = html
  const children: (Paragraph | Table)[] = []

  for (let i = 0; i < container.childNodes.length; i++) {
    const node = container.childNodes[i]
    const result = convertBlockNode(node)
    if (result) {
      if (Array.isArray(result)) {
        children.push(...result)
      } else {
        children.push(result)
      }
    }
  }

  return children
}

/** Convert a block-level node to docx elements. */
function convertBlockNode(node: ChildNode): Paragraph | Table | (Paragraph | Table)[] | null {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = node.textContent?.trim()
    if (!text) return null
    return new Paragraph({ children: [new TextRun(text)] })
  }

  if (node.nodeType !== Node.ELEMENT_NODE) return null

  const el = node as HTMLElement
  const tag = el.tagName.toLowerCase()

  switch (tag) {
    case 'h1':
    case 'h2':
    case 'h3':
    case 'h4':
    case 'h5':
    case 'h6': {
      const level = parseInt(tag[1])
      const headingMap: Record<number, HeadingLevel> = {
        1: HeadingLevel.HEADING_1,
        2: HeadingLevel.HEADING_2,
        3: HeadingLevel.HEADING_3,
        4: HeadingLevel.HEADING_4,
        5: HeadingLevel.HEADING_5,
        6: HeadingLevel.HEADING_6,
      }
      const runs = collectInlineTextRuns(el)
      return new Paragraph({
        children: runs,
        heading: headingMap[level],
        spacing: { before: 200, after: 100 },
      })
    }

    case 'p': {
      const runs = collectInlineTextRuns(el)
      if (runs.length === 0) return null
      return new Paragraph({
        children: runs,
        spacing: { before: 80, after: 80 },
      })
    }

    case 'pre': {
      return convertCodeBlock(el)
    }

    case 'blockquote': {
      const items: Paragraph[] = []
      for (let i = 0; i < el.childNodes.length; i++) {
        const child = el.childNodes[i]
        const para = convertBlockQuoteChild(child)
        if (para) items.push(para)
      }
      if (items.length === 0) return null
      return items
    }

    case 'ul':
    case 'ol': {
      const isOrdered = tag === 'ol'
      const items: Paragraph[] = []
      let counter = 1
      for (let i = 0; i < el.childNodes.length; i++) {
        const li = el.childNodes[i]
        if (li.nodeType !== Node.ELEMENT_NODE) continue
        if ((li as HTMLElement).tagName.toLowerCase() !== 'li') continue
        const prefix = isOrdered ? `${counter}. ` : '• '
        counter++
        const runs = collectInlineTextRuns(li as HTMLElement)
        if (runs.length === 0) continue
        items.push(
          new Paragraph({
            children: [new TextRun(prefix), ...runs],
            indent: { left: 360 },
            spacing: { before: 40, after: 40 },
          })
        )
      }
      if (items.length === 0) return null
      return items
    }

    case 'img': {
      // Standalone formula image（block-level）
      const src = el.getAttribute('src') || ''
      const alt = el.getAttribute('alt') || 'formula'
      const w = parseInt(el.getAttribute('width') || '200')
      const h = parseInt(el.getAttribute('height') || '40')
      if (src.startsWith('data:image/png;base64,')) {
        try {
          const base64 = src.replace('data:image/png;base64,', '')
          const binaryStr = atob(base64)
          const bytes = new Uint8Array(binaryStr.length)
          for (let j = 0; j < binaryStr.length; j++) bytes[j] = binaryStr.charCodeAt(j)
          const maxW = 500
          const ptW = Math.min(Math.round(w * 0.75), maxW)
          const ptH = Math.round((ptW / w) * h)
          return new Paragraph({
            children: [new ImageRun({
              type: 'png',
              data: bytes.buffer,
              transformation: { width: ptW, height: ptH },
              altText: { description: alt, title: alt, name: 'Formula' },
            }) as any],
            alignment: AlignmentType.CENTER,
            spacing: { before: 120, after: 120 },
          })
        } catch { /* fall through */ }
      }
      // Fallback: show alt text
      return new Paragraph({
        children: [new TextRun({ text: `[${alt}]`, italics: true, color: '999999' })],
        alignment: AlignmentType.CENTER,
      })
    }

    case 'table':
    case 'div': {
      // Check for katex-block wrapped formula image
      if (el.classList.contains('katex-block')) {
        const imgEl = el.querySelector('img[data-katex]')
        if (imgEl) return convertBlockNode(imgEl)
      }
      // Divs may wrap tables (e.g., .md-table-wrapper)
      const innerTable = el.querySelector('table')
      if (innerTable) return convertTableElement(innerTable)
      // Generic div: treat children as block
      const divChildren: (Paragraph | Table)[] = []
      for (let i = 0; i < el.childNodes.length; i++) {
        const result = convertBlockNode(el.childNodes[i])
        if (result) {
          if (Array.isArray(result)) {
            divChildren.push(...result)
          } else {
            divChildren.push(result)
          }
        }
      }
      return divChildren.length > 0 ? divChildren : null
    }

    case 'hr': {
      return new Paragraph({
        children: [new TextRun('─'.repeat(40))],
        alignment: AlignmentType.CENTER,
        spacing: { before: 120, after: 120 },
      })
    }

    default:
      return null
  }
}

/** Convert a <pre> code block to docx paragraphs. */
function convertCodeBlock(el: HTMLElement): Paragraph | null {
  // Skip the .code-header div inserted by markdown-it
  let codeText = ''
  const codeEl = el.querySelector('code')
  if (codeEl) {
    codeText = codeEl.textContent || ''
  } else {
    // Check if it's a mermaid block
    const mermaidEl = el.querySelector('pre.mermaid')
    if (mermaidEl) {
      codeText = mermaidEl.textContent || ''
    } else {
      codeText = el.textContent || ''
    }
  }

  if (!codeText.trim()) return null

  return new Paragraph({
    children: [new TextRun({ text: codeText, font: { name: 'Courier New' }, size: 18 })],
    shading: { fill: 'F5F5F5' },
    spacing: { before: 80, after: 80 },
    indent: { left: 120 },
  })
}

/** Convert a blockquote child to a docx paragraph. */
function convertBlockQuoteChild(node: ChildNode): Paragraph | null {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = node.textContent?.trim()
    if (!text) return null
    return new Paragraph({
      children: [new TextRun({ text, italics: true })],
      border: { left: { style: BorderStyle.SINGLE, size: 4, color: 'CCCCCC' } },
      indent: { left: 240 },
      spacing: { before: 40, after: 40 },
    })
  }

  if (node.nodeType !== Node.ELEMENT_NODE) return null
  const el = node as HTMLElement
  const runs = collectInlineTextRuns(el)
  if (runs.length === 0) return null

  return new Paragraph({
    children: runs.map(r => ({ ...r, italics: true })),
    border: { left: { style: BorderStyle.SINGLE, size: 4, color: 'CCCCCC' } },
    indent: { left: 240 },
    spacing: { before: 40, after: 40 },
  })
}

/** Convert an HTML <table> to a docx Table. */
function convertTableElement(table: HTMLTableElement): Table | null {
  const rows: TableRow[] = []

  // Detect header
  const thead = table.querySelector('thead')
  const tbody = table.querySelector('tbody')
  const hasHeader = !!thead

  const allRows = [
    ...(thead ? Array.from(thead.querySelectorAll('tr')) : []),
    ...(tbody ? Array.from(tbody.querySelectorAll('tr')) : []),
  ]

  // If no thead/tbody, use all tr elements directly
  if (allRows.length === 0) {
    const trs = table.querySelectorAll('tr')
    for (let i = 0; i < trs.length; i++) {
      const isHeader = i === 0 && trs.length > 1
      rows.push(buildTableRow(trs[i], isHeader))
    }
  } else {
    for (let i = 0; i < allRows.length; i++) {
      const isHeader = hasHeader && i < thead!.querySelectorAll('tr').length
      rows.push(buildTableRow(allRows[i], isHeader))
    }
  }

  if (rows.length === 0) return null

  return new Table({
    rows,
    width: { size: 100, type: WidthType.PERCENTAGE },
  })
}

function buildTableRow(tr: HTMLTableRowElement, isHeader: boolean): TableRow {
  const cells: TableCell[] = []
  const cellTag = isHeader ? 'th' : 'td'
  const children = tr.querySelectorAll(`${cellTag}, td, th`)

  for (let i = 0; i < children.length; i++) {
    const cell = children[i]
    const runs = collectInlineTextRuns(cell as HTMLElement)
    cells.push(
      new TableCell({
        children: runs.length > 0
          ? [new Paragraph({
              children: runs,
              ...(isHeader ? {} : {}),
            })]
          : [new Paragraph({ children: [] })],
        shading: isHeader ? { fill: 'EEEEEE' } : undefined,
      })
    )
  }

  return new TableRow({ children: cells })
}

/** Walk inline elements recursively and collect TextRun/ImageRun objects. */
function collectInlineTextRuns(el: HTMLElement, baseStyle: InlineStyle = DEFAULT_STYLE): (TextRun | ImageRun)[] {
  const runs: (TextRun | ImageRun)[] = []

  for (let i = 0; i < el.childNodes.length; i++) {
    const child = el.childNodes[i]

    if (child.nodeType === Node.TEXT_NODE) {
      const text = child.textContent || ''
      if (text) {
        const opts = styleToTextRunOptions(baseStyle)
        runs.push(new TextRun({ text, ...opts }))
      }
      continue
    }

    if (child.nodeType !== Node.ELEMENT_NODE) continue
    const ce = child as HTMLElement
    const tag = ce.tagName.toLowerCase()
    const style = cloneStyle(baseStyle)

    switch (tag) {
      case 'strong':
      case 'b':
        style.bold = true
        runs.push(...collectInlineTextRuns(ce, style))
        break

      case 'em':
      case 'i':
        style.italic = true
        runs.push(...collectInlineTextRuns(ce, style))
        break

      case 'u':
        style.underline = true
        runs.push(...collectInlineTextRuns(ce, style))
        break

      case 's':
      case 'del':
      case 'strike':
        style.strike = true
        runs.push(...collectInlineTextRuns(ce, style))
        break

      case 'code':
        style.font = 'Courier New'
        style.highlight = 'lightgray'
        runs.push(...collectInlineTextRuns(ce, style))
        break

      case 'a':
        style.underline = true
        style.color = '0044AA'
        runs.push(...collectInlineTextRuns(ce, style))
        break

      case 'span': {
        // Check for KaTeX spans: pass through as plain text
        if (ce.classList.contains('katex')) {
          const text = ce.textContent || ''
          if (text) {
            runs.push(new TextRun({ text, ...styleToTextRunOptions(style) }))
          }
          break
        }
        // Generic span
        runs.push(...collectInlineTextRuns(ce, style))
        break
      }

      case 'img': {
        const src = ce.getAttribute('src') || ''
        const alt = ce.getAttribute('alt') || 'formula'
        const w = parseInt(ce.getAttribute('width') || '100')
        const h = parseInt(ce.getAttribute('height') || '30')

        if (src.startsWith('data:image/png;base64,')) {
          try {
            const base64 = src.replace('data:image/png;base64,', '')
            const binaryStr = atob(base64)
            const bytes = new Uint8Array(binaryStr.length)
            for (let j = 0; j < binaryStr.length; j++) bytes[j] = binaryStr.charCodeAt(j)
            // px → pt 近似：1px ≈ 0.75pt，内联公式限制最大宽度
            const maxW = 400
            const ptW = Math.min(Math.round(w * 0.75), maxW)
            const ptH = Math.round((ptW / w) * h)
            runs.push(new ImageRun({
              type: 'png',
              data: bytes.buffer,
              transformation: { width: ptW, height: ptH },
              altText: { description: alt, title: alt, name: 'Formula' },
            }) as any)
          } catch {
            runs.push(new TextRun({ text: `[${alt}]`, italics: true, color: '999999' }))
          }
        } else {
          runs.push(new TextRun({ text: `[${alt}]`, italics: true, color: '999999' }))
        }
        break
      }

      case 'br':
        runs.push(new TextRun({ text: '\n', ...styleToTextRunOptions(style) }))
        break

      case 'div':
      case 'section':
        // Block elements inside inline context: recurse but treat as block text
        runs.push(...collectInlineTextRuns(ce, style))
        break

      default:
        // Other inline-like elements
        runs.push(...collectInlineTextRuns(ce, style))
        break
    }
  }

  return runs
}

export async function exportWord(
  content: string,
  renderHtml: (md: string) => string,
  control?: ExportControl,
): Promise<boolean> {
  assertExportCurrent(control)
  const rawHtml = renderHtml(content)
  // 将 KaTeX 公式渲染为 PNG 图片嵌入，使 Word 中公式显示为图片
  const html = await preprocessFormulas(rawHtml, control)
  assertExportCurrent(control)
  const children = parseHtmlToDocx(html)

  if (children.length === 0) {
    // Fallback: single paragraph with raw text
    children.push(
      new Paragraph({
        children: [new TextRun(content)],
      })
    )
  }

  const doc = new Document({
    sections: [
      {
        properties: {},
        children: children as any,
      },
    ],
  })

  const blob = await Packer.toBlob(doc)
  assertExportCurrent(control)

  const filePath = await save({
    title: '导出 Word',
    defaultPath: `fastclaw-answer_${timestamp()}.docx`,
    filters: [{ name: 'Word Document', extensions: ['docx'] }],
  })
  if (!filePath) return false

  assertExportCurrent(control)
  const bytes = await blob.arrayBuffer()
  assertExportCurrent(control)
  await writeFile(filePath, new Uint8Array(bytes))
  return true
}

// ── Tauri 原生导出（服务端渲染，支持 LaTeX 公式矢量/OMML）──────────

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
let tauriInvokeExport: InvokeFn | null = null

async function getInvoke(): Promise<InvokeFn | null> {
  if (tauriInvokeExport !== null) return tauriInvokeExport
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    tauriInvokeExport = invoke as InvokeFn
    return tauriInvokeExport
  } catch {
    return null
  }
}

/**
 * 通过 Rust 后端导出 DOCX（含可编辑 OMML 公式）。
 * 后端使用 jmisformu 将 Markdown → Word，LaTeX 公式转为 OMML。
 * @param content 原始 markdown 文本
 * @returns 是否成功
 */
export async function exportDocxNative(content: string): Promise<boolean> {
  const invoke = await getInvoke()
  if (!invoke) {
    throw new Error('Tauri 环境不可用')
  }

  const bytes: number[] = await invoke<number[]>('export_docx', { markdown: content })
  const filePath = await save({
    title: '导出 Word',
    defaultPath: `fastclaw-answer_${timestamp()}.docx`,
    filters: [{ name: 'Word Document', extensions: ['docx'] }],
  })
  if (!filePath) return false

  await writeFile(filePath, new Uint8Array(bytes))
  return true
}

/**
 * 通过 Rust 后端导出 PDF（含矢量公式）。
 * 后端使用 jmisformu 将 Markdown → PDF，LaTeX 公式用 RaTeX 渲染为矢量 SVG。
 * @param content 原始 markdown 文本
 * @returns 是否成功
 */
export async function exportPdfNative(content: string): Promise<boolean> {
  const invoke = await getInvoke()
  if (!invoke) {
    throw new Error('Tauri 环境不可用')
  }

  const bytes: number[] = await invoke<number[]>('export_pdf', { markdown: content })
  const filePath = await save({
    title: '导出 PDF',
    defaultPath: `fastclaw-answer_${timestamp()}.pdf`,
    filters: [{ name: 'PDF Document', extensions: ['pdf'] }],
  })
  if (!filePath) return false

  await writeFile(filePath, new Uint8Array(bytes))
  return true
}

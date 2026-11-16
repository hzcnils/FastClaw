export interface FastPlanOutputFile { name: string; url: string; size?: number; mime_type?: string; isArchive?: boolean }

/** Extract generated files from FastPlan node output without depending on UI state. */
export function detectFastPlanOutputFiles(content: unknown): FastPlanOutputFile[] {
  if (!content) return []
  const text = typeof content === 'string' ? content : JSON.stringify(content)
  try {
    const result = JSON.parse(text.trim())
    const files = collectStructuredFiles(result)
    if (files.length) return files
  } catch { /* Non-JSON output falls through to URL matching. */ }
  const match = text.match(/\/api\/(?:download|files)\/[^\s"'<>]+|\/api\/sessions\/[^/]+\/files\/[^\s"'<>]+|https?:\/\/[^\s"'<>]+\.(?:pdf|docx?|xlsx?|pptx?|txt|csv|json|xml|zip|tar\.gz)/i)
  if (!match) return []
  const url = match[0]
  let name = url.split('/').pop()?.split('?')[0] || '文件'
  try { name = decodeURIComponent(name) } catch {}
  return [{ name, url }]
}

function collectStructuredFiles(value: unknown): FastPlanOutputFile[] {
  const found = new Map<string, FastPlanOutputFile>()
  const visit = (current: any) => {
    if (!current || typeof current !== 'object') return
    if (Array.isArray(current)) { current.forEach(visit); return }

    const rawUrl = current.download_url || current.file_path || current.url
    const url = typeof rawUrl === 'string' ? rawUrl : ''
    const suppliedName = current.name || current.file_name || current.filename
    const looksLikeFile = !!url && (
      !!current.download_url || !!current.file_path || !!current.mime_type || !!current.mimeType || !!suppliedName
      || /\.(?:pdf|docx?|xlsx?|pptx?|txt|csv|json|xml|zip|tar\.gz)(?:\?|$)/i.test(url)
    )
    if (looksLikeFile) {
      let name = suppliedName || url.split('/').pop()?.split('?')[0] || '文件'
      try { name = decodeURIComponent(String(name)) } catch {}
      const candidate: FastPlanOutputFile = {
        name: String(name), url,
        size: finiteNumber(current.size),
        mime_type: current.mime_type || current.mimeType,
        isArchive: /\.(?:zip|tar\.gz)(?:\?|$)/i.test(url),
      }
      const previous = found.get(url)
      found.set(url, previous
        ? { ...candidate, ...previous, size: previous.size ?? candidate.size, mime_type: previous.mime_type || candidate.mime_type }
        : candidate)
    }
    Object.values(current).forEach(visit)
  }
  visit(value)
  return [...found.values()]
}

function finiteNumber(value: unknown) {
  const number = Number(value)
  return Number.isFinite(number) && number >= 0 ? number : undefined
}

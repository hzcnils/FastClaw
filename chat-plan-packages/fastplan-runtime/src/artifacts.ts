export interface FastPlanImageArtifact {
  src: string
  mimeType: string
}

const parsedJson = (value: string) => {
  const text = value.trim()
  if (!text || !((text.startsWith('{') && text.endsWith('}')) || (text.startsWith('[') && text.endsWith(']')))) return null
  try { return JSON.parse(text) } catch { return null }
}

export function isFastPlanEchartsOption(value: unknown): value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false
  const option = value as Record<string, any>
  const series = Array.isArray(option.series) ? option.series : option.series ? [option.series] : []
  if (!series.length) return false
  return series.some(item => typeof item?.type === 'string' && item.type.trim())
    || !!(option.xAxis || option.yAxis || option.radar || option.geo || option.polar || option.dataset)
}

function collectValues(raw: unknown, visit: (value: Record<string, any>) => boolean | void, depth = 0) {
  if (raw == null || depth > 8) return
  if (typeof raw === 'string') {
    const parsed = parsedJson(raw)
    if (parsed != null) collectValues(parsed, visit, depth + 1)
    return
  }
  if (Array.isArray(raw)) {
    raw.forEach(item => collectValues(item, visit, depth + 1))
    return
  }
  if (typeof raw !== 'object') return
  const value = raw as Record<string, any>
  if (visit(value)) return
  Object.values(value).forEach(item => collectValues(item, visit, depth + 1))
}

export function extractFastPlanImages(raw: unknown): FastPlanImageArtifact[] {
  const artifacts: FastPlanImageArtifact[] = []
  const seen = new Set<string>()
  collectValues(raw, value => {
    const type = String(value.type || '').toLowerCase()
    const declaredMimeType = value.mimeType || value.media_type || value.mime_type || value.source?.media_type || ''
    if (type !== 'image' && type !== 'image_url' && !String(declaredMimeType).startsWith('image/')) return false
    const mimeType = declaredMimeType || 'image/png'
    const nestedUrl = typeof value.image_url === 'object' ? value.image_url?.url : value.image_url
    const candidate = value.data || value.base64 || value.url || nestedUrl || value.source?.data
    if (typeof candidate !== 'string' || !candidate.trim()) return true
    const clean = candidate.trim()
    const src = /^(data:image|https?:\/\/|blob:)/.test(clean) ? clean : `data:${mimeType};base64,${clean}`
    const key = src.length > 160 ? `${src.length}:${src.slice(0, 64)}:${src.slice(-32)}` : src
    if (!seen.has(key)) { seen.add(key); artifacts.push({ src, mimeType }) }
    return true
  })
  return artifacts
}

import { isFastPlanEchartsOption } from './artifacts'

export interface FastPlanFinalAnswerInput {
  kind: 'final_answer'
  content: string
}

export type FastPlanAnswerPart =
  | { type: 'text'; text: string }
  | { type: 'chart'; option: Record<string, unknown>; source: string }

/** Only standalone blocks in final-answer text are eligible. Never scan wrappers or logs. */
function splitAnswerText(text: string): FastPlanAnswerPart[] {
  const matches: Array<{ start: number; end: number; option: Record<string, unknown> }> = []
  const lines = text.match(/[^\n]*\n|[^\n]+$/g) || []
  let offset = 0
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].replace(/\r?\n$/, '')
    // Consume ALL fences, including examples and unfinished fences, so their JSON is not rescanned.
    const fence = /^(\s*)(`{3,}|~{3,})(.*)$/.exec(line)
    if (fence) {
      const start = offset
      const marker = fence[2]
      let body = '', closed = false
      offset += lines[i].length
      while (++i < lines.length) {
        const next = lines[i].replace(/\r?\n$/, '')
        offset += lines[i].length
        const closing = next.trim()
        if (closing.length >= marker.length && [...closing].every(char => char === marker[0])) { closed = true; break }
        body += lines[i]
      }
      if (closed && !fence[1] && /^(echarts|json)?$/i.test(fence[3].trim())) {
        try {
          const option = JSON.parse(body)
          if (isFastPlanEchartsOption(option)) matches.push({ start, end: offset, option })
        } catch { /* Keep invalid JSON verbatim. */ }
      }
      continue
    }
    // A bare option must occupy a standalone paragraph, never inline prose or a nested field.
    if (line.startsWith('{') && (i === 0 || !lines[i - 1].trim())) {
      let body = '', end = offset
      for (let j = i; j < lines.length && lines[j].trim(); j++) {
        body += lines[j]; end += lines[j].length
        try {
          const option = JSON.parse(body)
          if ((j === lines.length - 1 || !lines[j + 1].trim()) && isFastPlanEchartsOption(option)) {
            matches.push({ start: offset, end, option })
            i = j; offset = end - lines[j].length
            break
          }
        } catch { /* Wait for a complete object. */ }
      }
    }
    offset += lines[i].length
  }
  const parts: FastPlanAnswerPart[] = []
  let cursor = 0
  for (const match of matches) {
    if (match.start > cursor) parts.push({ type: 'text', text: text.slice(cursor, match.start) })
    parts.push({ type: 'chart', option: match.option, source: text.slice(match.start, match.end) })
    cursor = match.end
  }
  if (cursor < text.length) parts.push({ type: 'text', text: text.slice(cursor) })
  return parts
}

/** Refuses untyped strings so node output/log data cannot enter chart extraction accidentally. */
export function splitFastPlanFinalAnswer(input: FastPlanFinalAnswerInput): FastPlanAnswerPart[] {
  if (!input || input.kind !== 'final_answer' || typeof input.content !== 'string') return []
  return splitAnswerText(input.content)
}

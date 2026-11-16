export function resolveFastPlanDownloadUrl(url: string, downloadBase = '') {
  if (!url || /^https?:/i.test(url) || !downloadBase) return url
  return `${downloadBase.replace(/\/$/, '')}/${url.replace(/^\//, '')}`
}

export function normalizeFastPlanSubmission(value: unknown, fallback = '') {
  if (typeof value === 'string') return value.trim()
  if (value && typeof value === 'object') {
    const record = value as { text?: unknown; value?: unknown; content?: unknown }
    const content = record.text ?? record.value ?? record.content
    if (typeof content === 'string') return content.trim()
  }
  return fallback.trim()
}

export function resolveFastPlanComposerAttachments(attachments: any[] = []) {
  if (!Array.isArray(attachments)) return []
  return attachments.flatMap((attachment) => {
    if (!attachment || typeof attachment !== 'object') return []
    const status = String(attachment.status || '').toLowerCase()
    if (status && !['complete', 'completed', 'success', 'ready', 'done'].includes(status)) return []

    const absPath = attachment.abs_path ?? attachment.absPath ?? attachment.path
    if (typeof absPath !== 'string' || !absPath.trim()) return []
    const filename = String(attachment.filename || attachment.name || absPath.split(/[\\/]/).pop() || '').trim()
    if (!filename) return []
    const ext = String(attachment.ext || filename.match(/\.([^./\\]+)$/)?.[1] || '').toLowerCase()
    return [{ filename, abs_path: absPath.trim(), ...(ext ? { ext } : {}) }]
  })
}

export function withFastPlanAssistantShell(messages: any[], plan: unknown, model: Record<string, unknown>, history: any[] = []) {
  // A cancelled/failed turn can have a plan but no persisted assistant reply.
  // Give it the same stable mount point as a live planning placeholder.
  if (history.length) {
    messages = [...messages]
    for (const entry of history) {
      const start = messages.findIndex(message => message.role === 'user' && String(message.id) === String(entry.anchorMessageId))
      if (start < 0) continue
      const next = messages.findIndex((message, index) => index > start && message.role === 'user')
      const end = next < 0 ? messages.length : next
      if (messages.slice(start + 1, end).some(message => message.role === 'assistant')) continue
      messages.splice(end, 0, { protocolVersion: 1, id: `temporary-fastplan-planning-${String(entry.anchorMessageId)}`,
        role: 'assistant', createdAt: messages[start].createdAt, model, status: 'complete', parts: [],
        metadata: { fastPlanPlanningShell: true, anchorMessageId: entry.anchorMessageId } })
    }
  }
  if (!plan) return messages
  let userIndex = -1
  for (let index = messages.length - 1; index >= 0; index--) {
    if (messages[index]?.role === 'user') { userIndex = index; break }
  }
  if (userIndex < 0 || messages.slice(userIndex + 1).some(message => message?.role === 'assistant')) return messages

  const userMessage = messages[userIndex]
  return [...messages, {
    protocolVersion: 1,
    id: `temporary-fastplan-planning-${String(userMessage.id)}`,
    role: 'assistant',
    createdAt: userMessage.createdAt ?? Date.now(),
    model,
    status: 'pending',
    parts: [],
    turnId: userMessage.turnId,
    metadata: { fastPlanPlanningShell: true, anchorMessageId: userMessage.id },
  }]
}

export function findFastPlanFinalAssistantMessageId(messages: any[]) {
  return [...messages].reverse().find(message => message?.role === 'assistant'
    && message.metadata?.fastPlanPlanningShell !== true
    && message.metadata?.contentKind === 'final_answer')?.id
}

/**
 * Completed FastPlan turns can contain several persisted assistant progress
 * messages followed by one authoritative final answer. Keep those progress
 * messages available, but let the renderer mount their Markdown only on demand.
 * Active/incomplete turns have no final answer and are intentionally untouched.
 */
export function collectDeferredFastPlanMessageIds(messages: any[]) {
  const deferred = new Set<string>()
  let turn: any[] = []
  const flush = () => {
    const hasFinalAnswer = turn.some(message => message?.role === 'assistant'
      && message.metadata?.contentKind === 'final_answer')
    if (hasFinalAnswer) {
      for (const message of turn) {
        if (message?.role !== 'assistant'
          || message.status === 'error'
          || message.metadata?.fastPlanPlanningShell === true
          || message.metadata?.contentKind === 'final_answer') continue
        if ((message.parts || []).some((part: any) => part?.type === 'text' && String(part.text || '').trim())) {
          deferred.add(String(message.id))
        }
      }
    }
    turn = []
  }

  for (const message of messages || []) {
    if (message?.role === 'user') flush()
    turn.push(message)
  }
  flush()
  return deferred
}

export function summarizeFastPlanProcessText(value: unknown, maxChars = 140) {
  const text = String(value || '')
    .replace(/```[\s\S]*?```/g, ' [代码] ')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, ' [图片] ')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}(?:#{1,6}|[-+*>]|\d+\.)\s+/gm, '')
    .replace(/[`*_~|]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  if (text.length <= maxChars) return text
  return `${text.slice(0, Math.max(0, maxChars - 1)).trimEnd()}…`
}

const themeTokenProperties: Record<string, string> = {
  background: '--jcr-bg', foreground: '--jcr-fg', muted: '--jcr-muted', border: '--jcr-border', primary: '--jcr-primary',
  userBackground: '--jcr-user-bg', assistantBackground: '--jcr-assistant-bg', danger: '--jcr-danger', codeBackground: '--jcr-code-bg',
  surface: '--fp-surface', subtleBackground: '--fp-subtle-bg', radius: '--jcr-radius', fontSize: '--jcr-font-size',
  gap: '--jcr-gap', bubbleMaxWidth: '--jcr-bubble-max-width',
}

export function createFastPlanThemeStyle(tokens: Record<string, string | undefined> = {}) {
  return Object.fromEntries(Object.entries(tokens)
    .filter(([, value]) => value !== undefined && value !== '')
    .map(([key, value]) => [themeTokenProperties[key] || key, value]))
}

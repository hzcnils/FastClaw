/** FastPlan reasoning/tool-log normalization shared by WS, SSE and any host UI. */
export function parseFastPlanThinking(content, phase = null) {
  const text = String(content || '').trim()
  if (!text) return null
  if (phase) {
    if (typeof phase === 'object') return { type: 'divider', content: `[${phase.custom?.name || JSON.stringify(phase)}] ${text}` }
    if (phase === 'searching') return { type: 'tool_call', toolName: text.match(/正在调用工具[:：]\s*(.+)/)?.[1]?.trim() || '', content: text }
    if (['analyzing', 'planning', 'generating', 'validating', 'reflecting'].includes(phase)) return { type: 'reasoning', content: text }
    return { type: 'info', content: text }
  }
  if (text.startsWith('🔨') && text.includes('工具调用')) {
    const toolName = text.match(/├─\s*(?:工具|tool)\s*:\s*(.+)/)?.[1]?.trim() || ''
    const reason = text.match(/[├└]─\s*(?:原因|reasoning)\s*:\s*(.+)/)?.[1]?.trim()
    const args = text.match(/└─\s*(?:args|参数)\s*:\s*([\s\S]+)$/)?.[1]?.trim()
    let pretty = args || ''
    try { if (args) pretty = JSON.stringify(JSON.parse(args), null, 2) } catch {}
    return { type: 'tool_call', toolName, content: [reason, pretty].filter(Boolean).join('\n') || text.split('\n').slice(1).join('\n').trim() }
  }
  if (text.startsWith('✅') && (text.includes('工具调用成功') || text.includes('工具成功'))) return { type: 'tool_result', toolName: text.match(/├─\s*(?:工具|tool)\s*:\s*(.+)/)?.[1]?.trim() || '', content: text }
  if (text.startsWith('🏁')) return { type: 'result', content: text }
  if (/^[⚠️❌🚫❓]/.test(text)) return { type: 'error', content: text }
  if (/^(?:📄|🛠️|📦|🧠|🔍|📚|🤖|📋|🚀|📝|✅|🔧|🔄|⚡|📊|⏳)/.test(text)) return { type: 'info', content: text }
  return { type: 'reasoning', content: text }
}

export function pushFastPlanLog(logs, entry) {
  if (!entry) return
  const last = logs[logs.length - 1]
  if (entry.type === 'reasoning' && last?.type === 'reasoning') { last.content += entry.content; return }
  logs.push(entry)
}

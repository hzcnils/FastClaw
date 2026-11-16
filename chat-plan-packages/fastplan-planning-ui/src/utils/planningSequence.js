const settledStatuses = new Set(['done', 'completed', 'success', 'failed', 'error'])

function settled(row) {
  return settledStatuses.has(String(row?.status || '').toLowerCase())
}

/**
 * Keeps planning stages visually causal when transport events arrive out of order.
 * Retrieval and later stages stay buffered until the displayed intent stage settles.
 */
export function sequencePlanningRows(rows = [], { planningActive = false } = {}) {
  const intent = rows.find(row => row?.stage === 'intent')
  if (intent) return settled(intent) ? rows : [intent]
  if (!planningActive || rows.length === 0) return rows
  return [{
    id: 'planning:intent:waiting',
    stage: 'intent',
    status: 'running',
    content: '正在理解用户意图…',
    synthetic: true,
  }]
}

const OBJECT_STRING = '[object Object]'

export interface FastPlanNormalizedError extends Error {
  code?: string
  recoverable?: boolean
  terminal?: boolean
  cause?: unknown
  details?: unknown
}

function meaningfulText(value: unknown) {
  if (typeof value !== 'string') return ''
  const text = value.trim()
  return text && text !== OBJECT_STRING ? text : ''
}

function findErrorMessage(value: unknown, seen: Set<object>): string {
  const direct = meaningfulText(value)
  if (direct) return direct
  if (!value || typeof value !== 'object' || seen.has(value)) return ''
  seen.add(value)

  const record = value as Record<string, unknown>
  for (const key of ['message', 'errorMessage', 'error', 'detail', 'reason', 'msg', 'cause', 'data', 'result', 'response']) {
    const message = findErrorMessage(record[key], seen)
    if (message) return message
  }
  return ''
}

/** Extracts a readable message without coercing protocol event objects to `[object Object]`. */
export function fastPlanErrorMessage(value: unknown, fallback = 'FastPlan 执行失败') {
  return findErrorMessage(value, new Set()) || fallback
}

export function userFacingTaskError(raw: unknown) {
  const text = String(raw || '').replace(/\s+/g, ' ').trim()
  if (!text) return ''
  if (/HTTP\s*401|invalid_api_key|authentication failed/i.test(text)) return '模型鉴权失败，请检查模型密钥后再试'
  if (/HTTP\s*403|access denied|permission denied/i.test(text)) return '模型调用失败，请检查模型访问权限后再试'
  if (/HTTP\s*400|request rejected/i.test(text)) return '模型服务拒绝了本次请求，请稍后重试'
  if (/HTTP\s*429/i.test(text)) return '模型请求过于频繁，请稍后重试'
  if (/HTTP\s*5\d\d|unavailable|timeout|ECONN/i.test(text)) return '模型服务暂时不可用，请稍后重试'
  if (/LLM error|Request failed|model service/i.test(text)) {
    const chinese = text.replace(/LLM error:\s*/ig, '').replace(/Request failed:[^]*/i, '').replace(/Agent LLM\s*/ig, '').replace(/[:：]+$/g, '').trim()
    return /[\u4e00-\u9fff]/.test(chinese) ? chinese : '模型调用失败，请稍后重试'
  }
  return text
}

/** Converts transport, protocol and host errors to the single error contract emitted by the runtime. */
export function normalizeFastPlanError(value: unknown, fallback = 'FastPlan 执行失败'): FastPlanNormalizedError {
  const record = value && typeof value === 'object' ? value as Record<string, unknown> : {}
  const message = fastPlanErrorMessage(value, fallback)
  if (value instanceof Error && meaningfulText(value.message) === message) {
    return value as FastPlanNormalizedError
  }

  const normalized = new Error(message) as FastPlanNormalizedError
  if (record.code !== undefined && record.code !== null && String(record.code).trim()) normalized.code = String(record.code)
  if (typeof record.recoverable === 'boolean') normalized.recoverable = record.recoverable
  if (typeof record.terminal === 'boolean') normalized.terminal = record.terminal
  normalized.cause = value
  normalized.details = value
  return normalized
}

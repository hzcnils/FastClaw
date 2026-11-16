import type { ChatErrorAction, ChatErrorCategory, ChatMessageError } from './types'

export interface ResolvedChatError extends ChatMessageError {
  category: ChatErrorCategory
  actions: ChatErrorAction[]
}

/**
 * Normalize the legacy one-line transport error emitted by FastClaw into the
 * same field-oriented layout as HTTP response errors. The original value is
 * still used for classification and semantic actions.
 */
export function formatChatErrorRaw(raw: string): string {
  const requestFailure = raw.match(/^(.*?)请求\s+(https?:\/\/\S+)\s+失败（模型\s+(.+?)\s+\/\s+(.+?)）:\s*([\s\S]+)$/i)
  if (!requestFailure) return raw

  const [, prefix, requestUrl, model, provider, detail] = requestFailure
  let providerBaseUrl = requestUrl
  try {
    const parsed = new URL(requestUrl)
    providerBaseUrl = `${parsed.origin}${parsed.pathname.replace(/\/chat\/completions\/?$/, '')}`
  } catch {
    // Keep the request URL if a non-standard host URL cannot be parsed.
  }

  return `${prefix}LLM 请求错误\n模型: ${model.trim()}\n供应商：${provider.trim()} (${providerBaseUrl})\n请求地址:${requestUrl}\n错误详情:\n${detail}`
}

const QUOTA_PATTERN = /(?:insufficient[_\s-]*(?:user[_\s-]*)?(?:quota|balance|credit)|quota\s*(?:exceeded|exhausted|insufficient)|(?:balance|credit)\s*(?:insufficient|exhausted)|billing\s*(?:limit|quota)|用户(?:额度|余额)不足|额度不足|余额不足|配额不足|欠费)/i
const PERMISSION_PATTERN = /(?:permission\s*denied|insufficient\s*permissions?|access\s*denied|forbidden\s*(?:model|endpoint)|not\s*allowed|无权访问|权限不足|没有.*权限|未开通.*模型)/i
const ACCOUNT_RESTRICTED_PATTERN = /(?:account\s*(?:is\s*)?(?:suspended|disabled|blocked|restricted|frozen)|risk\s*control|账号.*(?:冻结|禁用|封禁|受限)|账户.*(?:冻结|禁用|封禁|受限)|风控)/i
const AUTH_PATTERN = /(?:invalid\s+(?:api\s+)?(?:key|token)|authentication\s+fails?|unauthori[sz]ed|authentication\s*(?:failed|error)|未授权|无效的?\s*(?:api\s*)?(?:密钥|token)|密钥.*(?:错误|无效|失效))/i
const NETWORK_PATTERN = /(?:failed\s+to\s+fetch|error\s+sending\s+request|client\s+error\s*\(sendrequest\)|connection\s+(?:refused|reset|failed|closed)|incomplete\s+message|tcp\s+connect|network\s+error|dns|os\s+error\s+(?:10054|10060|10065)|无法连接|连接失败|连接已关闭|网络错误|网络连接)/i
const TIMEOUT_PATTERN = /(?:timed?\s*out|timeout|超时)/i

export function extractChatErrorStatusCode(raw: string): number | undefined {
  const patterns = [
    /HTTP\s*状态码\s*[:：]?\s*(\d{3})/i,
    /HTTP\s*(?:status(?:\s*code)?\s*[:：=]?\s*)?(\d{3})/i,
    /["'](?:status|status_code|statusCode|code)["']\s*:\s*(\d{3})/i,
  ]
  for (const pattern of patterns) {
    const matched = raw.match(pattern)
    if (matched) return Number(matched[1])
  }
  return undefined
}

function defaultsFor(raw: string, statusCode?: number): Pick<ResolvedChatError, 'category' | 'message' | 'actions'> {
  if (statusCode === 401 || AUTH_PATTERN.test(raw)) {
    return { category: 'authentication', message: '请确认当前配置的密钥是否正确', actions: [] }
  }
  if (QUOTA_PATTERN.test(raw)) {
    return {
      category: 'quota',
      message: '当前模型账户额度或余额不足',
      actions: [],
    }
  }
  if (statusCode === 403) {
    if (ACCOUNT_RESTRICTED_PATTERN.test(raw)) {
      return { category: 'account-restricted', message: '当前账号可能受限或已被禁用，请联系服务提供方', actions: [] }
    }
    if (PERMISSION_PATTERN.test(raw)) {
      return { category: 'permission', message: '当前密钥没有访问该模型或接口的权限', actions: [] }
    }
    return { category: 'permission', message: '当前请求被服务拒绝，请检查账号权限、模型访问权限、IP或地域限制', actions: [] }
  }
  if (statusCode === 404) {
    return { category: 'model-unavailable', message: '未找到模型或接口，请检查模型名称和 API 地址', actions: [] }
  }
  if (statusCode === 408 || statusCode === 504 || TIMEOUT_PATTERN.test(raw)) {
    return { category: 'timeout', message: '模型服务响应超时，请稍后重试', actions: [] }
  }
  if (statusCode === 429) {
    return { category: 'rate-limit', message: '请求过于频繁，请稍后重试', actions: [] }
  }
  if (statusCode === 500) {
    return { category: 'server', message: '模型服务内部异常，请稍后重试', actions: [] }
  }
  if (statusCode === 502) {
    return { category: 'gateway', message: '模型服务暂时无法访问，请确认接口地址及网络是否可访问', actions: [] }
  }
  if (statusCode === 503) {
    return { category: 'model-unavailable', message: '当前模型不可用，请重新检查模型信息', actions: [] }
  }
  if (NETWORK_PATTERN.test(raw)) {
    return { category: 'network', message: '无法访问模型服务，请检查网络和接口地址', actions: [] }
  }
  return { category: 'unknown', message: undefined, actions: [] }
}

export function resolveChatError(error: ChatMessageError | string): ResolvedChatError {
  const source: ChatMessageError = typeof error === 'string' ? { raw: error } : error
  const raw = source.raw || ''
  const statusCode = source.statusCode ?? extractChatErrorStatusCode(raw)
  const defaults = defaultsFor(raw, statusCode)
  return {
    ...source,
    raw,
    statusCode,
    category: source.category ?? defaults.category,
    message: source.message ?? defaults.message,
    actions: source.actions ?? defaults.actions,
  }
}

import { ref } from 'vue'
import type { ChatMessage, ChatParameterField, ChatParameterValues } from 'chat-renderer'

export const providerParameterFields: ChatParameterField[] = [
  { key: 'baseUrl', label: 'Base URL', type: 'url', placeholder: 'https://api.example.com/v1', required: true },
  { key: 'apiKey', label: 'API Key', type: 'password', placeholder: 'API Key', required: true },
]

export interface ProviderCompletionConfig {
  id: string
  name: string
  baseUrl: string
  apiKey: string
}

/** A transient gate: no credentials or unfinished requests enter message persistence. */
export function createProviderCompletion(options: {
  load: (modelId: string) => Promise<ProviderCompletionConfig | null>
  save: (provider: ProviderCompletionConfig, values: ChatParameterValues) => Promise<void>
}) {
  const pending = ref<{ id: string; createdAt: number; modelId: string; userMessage?: ChatMessage; provider: ProviderCompletionConfig; values: ChatParameterValues } | null>(null)
  const saving = ref(false)
  const error = ref('')
  let generation = 0
  let resolvePending: ((ready: boolean) => void) | undefined
  let active: { modelId: string; promise: Promise<boolean> } | undefined

  function cancel() {
    generation++
    pending.value = null
    error.value = ''
    saving.value = false
    resolvePending?.(false)
    resolvePending = undefined
    active = undefined
  }

  function ensureReady(modelId: string, request?: { message: ChatMessage; onRequired: () => void }, signal?: AbortSignal): Promise<boolean> {
    if (signal?.aborted) return Promise.resolve(false)
    // Only the original operation may resume; additional clicks must not queue sends.
    if (active) return Promise.resolve(false)
    const version = generation
    const cancelOwnRequest = () => { if (active?.promise === promise) cancel() }
    const promise = (async () => {
      const provider = await options.load(modelId)
      if (generation !== version) return false
      if (!provider || (provider.baseUrl.trim() && provider.apiKey.trim())) return true
      pending.value = {
        id: crypto.randomUUID(), createdAt: Date.now(), modelId, provider, userMessage: request?.message,
        values: { baseUrl: provider.baseUrl, apiKey: provider.apiKey },
      }
      error.value = ''
      const completion = new Promise<boolean>(resolve => { resolvePending = resolve })
      request?.onRequired()
      return completion
    })().catch(cause => {
      if (generation !== version) return false
      throw cause
    }).finally(() => {
      signal?.removeEventListener('abort', cancelOwnRequest)
      if (active?.promise === promise) active = undefined
    })
    active = { modelId, promise }
    signal?.addEventListener('abort', cancelOwnRequest, { once: true })
    if (signal?.aborted) cancelOwnRequest()
    return promise
  }

  async function submit(values: ChatParameterValues) {
    const request = pending.value
    if (!request || saving.value) return
    const normalized = { baseUrl: (values.baseUrl || '').trim(), apiKey: (values.apiKey || '').trim() }
    if (!normalized.baseUrl || !normalized.apiKey) {
      error.value = '请完整填写 Base URL 和 API Key'
      return
    }
    try {
      const url = new URL(normalized.baseUrl)
      if (!['http:', 'https:'].includes(url.protocol) || !url.hostname) throw new Error()
    } catch {
      error.value = '请填写有效的 HTTP/HTTPS Base URL'
      return
    }
    saving.value = true
    error.value = ''
    try {
      await options.save(request.provider, normalized)
      if (pending.value?.id !== request.id) return
      pending.value = null
      resolvePending?.(true)
      resolvePending = undefined
    } catch (cause) {
      if (pending.value?.id === request.id) error.value = typeof cause === 'string' ? cause : (cause as Error)?.message || '保存失败，请重试'
    } finally {
      if (!pending.value || pending.value.id === request.id) saving.value = false
    }
  }

  return { pending, saving, error, ensureReady, submit, cancel }
}

import type { ChatMessage, ChatPart, ToolCallPart } from 'chat-renderer'
import { splitFastPlanFinalAnswer } from 'planflow-runtime'
import type { Message } from '@/types'

export interface MessageAdapterOptions {
  modelName?: (modelId: string | null) => string | undefined
  modelAvatar?: (modelId: string | null) => string | undefined
}

function messageStatus(status: Message['status']): ChatMessage['status'] {
  if (status === 'completed' || status === 'canceled') return 'complete'
  return status
}

function toolStatus(status: 'running' | 'done' | 'error'): ToolCallPart['status'] {
  if (status === 'done') return 'complete'
  return status
}

type UnknownRecord = Record<string, unknown>

function asRecord(value: unknown): UnknownRecord | undefined {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? value as UnknownRecord
    : undefined
}

function parseJsonRecord(value: unknown): UnknownRecord | undefined {
  if (typeof value !== 'string' || !value.trim()) return asRecord(value)
  try { return asRecord(JSON.parse(value)) } catch { return undefined }
}

/**
 * FastClaw persists tool results as an outer invocation envelope whose `result`
 * may itself be a serialized MCP/tool response. Keep the public renderer free
 * from that host-specific storage shape and expose only the useful payload.
 */
function unwrapToolOutput(value: unknown): unknown {
  const parsed = typeof value === 'string' ? parseJsonRecord(value) : asRecord(value)
  if (!parsed) return value

  const isResponseEnvelope = 'content' in parsed
    && ('is_error' in parsed || 'isError' in parsed || 'metadata' in parsed)
  return isResponseEnvelope ? parsed.content : parsed
}

function persistedToolPart(message: Message): ToolCallPart | undefined {
  if (message.role !== 'tool') return undefined

  const runtimeMessage = message as Message & {
    tool_name?: string
    tool_input?: unknown
    tool_call_id?: string
  }
  const envelope = parseJsonRecord(message.content)
  const name = typeof envelope?.tool_name === 'string'
    ? envelope.tool_name
    : runtimeMessage.tool_name
  if (!name) return undefined

  const id = typeof envelope?.tool_call_id === 'string'
    ? envelope.tool_call_id
    : runtimeMessage.tool_call_id || message.id
  const input = envelope && 'arguments' in envelope
    ? envelope.arguments
    : runtimeMessage.tool_input
  const rawOutput = envelope && 'result' in envelope
    ? envelope.result
    : message.content
  const nestedResult = parseJsonRecord(rawOutput)
  const failed = message.status === 'error'
    || envelope?.success === false
    || nestedResult?.is_error === true
    || nestedResult?.isError === true
  const output = unwrapToolOutput(rawOutput)

  return {
    type: 'tool-call',
    id,
    name,
    input,
    output,
    status: message.status === 'streaming' || message.status === 'pending'
      ? 'running'
      : failed ? 'error' : 'complete',
    error: failed
      ? (typeof output === 'string' ? output : JSON.stringify(output, null, 2))
      : undefined,
  }
}

function attachmentParts(message: Message): ChatPart[] {
  return (message.attachments || []).map((attachment): ChatPart => {
    if (attachment.type === 'image' || attachment.type === 'screenshot') {
      return {
        type: 'image',
        src: attachment.preview_url || '',
        name: attachment.name,
        alt: attachment.name,
      }
    }
    return {
      type: 'attachment',
      id: attachment.id,
      name: attachment.name,
      url: attachment.preview_url,
      size: attachment.size,
      metadata: { hostAttachment: attachment },
    }
  }).filter(part => part.type !== 'image' || Boolean(part.src))
}

function toolParts(message: Message): ChatPart[] {
  const parts: ChatPart[] = []
  const persistedPart = persistedToolPart(message)
  if (persistedPart) parts.push(persistedPart)
  if (message.tool_call) {
    parts.push({
      type: 'tool-call',
      id: `direct:${message.id}`,
      name: message.tool_call.tool_name,
      input: message.tool_call.arguments,
      status: message.status === 'error' ? 'error' : 'complete',
      error: message.status === 'error' ? message.content : undefined,
    })
  }
  for (const call of message.tool_calls || []) {
    parts.push({
      type: 'tool-call',
      id: call.id,
      name: call.name,
      input: call.args,
      output: call.result_preview,
      status: toolStatus(call.status),
      error: call.status === 'error' ? call.result_preview : undefined,
    })
  }
  return parts
}

/** 对齐 FastPlanAnswer：正文中的 ```echarts / 合法图表 JSON 拆成自定义图表部件。 */
function contentParts(content: string): ChatPart[] {
  const text = String(content || '')
  if (!text) return []
  const split = splitFastPlanFinalAnswer({ kind: 'final_answer', content: text })
  if (!split.some((part) => part.type === 'chart')) {
    return [{ type: 'text', text, format: 'markdown' }]
  }
  const parts: ChatPart[] = []
  for (const part of split) {
    if (part.type === 'chart') {
      parts.push({
        type: 'custom',
        kind: 'fastclaw-echarts',
        data: { option: part.option, source: part.source },
      })
      continue
    }
    if (String(part.text || '').trim()) {
      parts.push({ type: 'text', text: part.text, format: 'markdown' })
    }
  }
  return parts.length ? parts : [{ type: 'text', text, format: 'markdown' }]
}

export function adaptMessage(message: Message, options: MessageAdapterOptions = {}): ChatMessage {
  const parts: ChatPart[] = []
  if (message.think_content?.trim()) {
    parts.push({
      type: 'reasoning',
      text: message.think_content || '',
      status: message.status === 'streaming' && (message.stream_phase === 'reasoning'
        || (!message.stream_phase && !message.content)) ? 'streaming' : 'complete',
    })
  }
  if (message.planRuntime) {
    parts.push({
      type: 'custom',
      kind: 'fastclaw-plan',
      data: {
        runtime: message.planRuntime,
        sessionId: message.conversation_id,
      },
    })
  }
  const tools = toolParts(message)
  parts.push(...tools)
  if (message.status === 'streaming' && message.isThinking && !message.content && !message.think_content?.trim() && !message.planRuntime) {
    parts.push({ type: 'notice', text: message.stream_phase === 'finalizing' ? '正在生成最终回答…' : '等待模型响应…' })
  }
  parts.push(...attachmentParts(message))
  // A recognized role=tool message is represented by its structured tool part;
  // rendering the persisted JSON envelope again would leak transport data.
  if (message.status !== 'error' && message.content && !(message.role === 'tool' && tools.length)) {
    parts.push(...contentParts(message.content))
  }
  if (message.web_searching || message.web_hits?.length) {
    parts.push({ type: 'custom', kind: 'fastclaw-web-search', data: {
      messageId: message.id, searching: Boolean(message.web_searching), query: message.web_search_query, hits: message.web_hits || [],
    } })
  }
  if (message.skill_used) parts.push({ type: 'custom', kind: 'fastclaw-skill-used', data: message.skill_used })
  if (message._vendorConfig) {
    parts.push({ type: 'custom', kind: 'fastclaw-vendor-config', data: { providerCode: message._providerCode } })
  }
  if (!parts.length && message.status !== 'error') parts.push({ type: 'text', text: '', format: 'markdown' })

  const adapted: ChatMessage = {
    id: message.id,
    role: message.role,
    createdAt: message.created_at || 0,
    status: messageStatus(message.status),
    error: message.status === 'error' ? { raw: message.content } : undefined,
    retry: message.status === 'streaming' ? message.retry_progress : undefined,
    parts,
    feedback: message.is_liked === -1 ? -1 : message.is_liked === 1 ? 1 : 0,
    turnId: message.group_id || undefined,
    metadata: {
      hostStatus: message.status,
      contextCleared: Boolean(message.context_cleared),
      favorite: message.is_favorite === 1,
      translating: Boolean(message.translating),
      translationLang: message.translation_lang,
      traceSource: message.trace_source,
      agentName: message.agent_name,
      agentIcon: message.agent_icon,
      vendorConfig: Boolean(message._vendorConfig),
      providerCode: message._providerCode,
    },
  }
  if (message.model_id || message.agent_name || message.agent_icon) {
    const resolvedName = message.agent_name
      || options.modelName?.(message.model_id)
      || message.model_id
      || undefined
    adapted.model = {
      id: message.model_id || undefined,
      name: resolvedName,
      avatar: message.agent_icon || options.modelAvatar?.(message.model_id),
    }
  }
  if (message.translation) {
    adapted.translations = [{
      id: `${message.id}:translation:${message.translation_lang || 'unknown'}`,
      role: 'assistant',
      createdAt: message.edited_at || message.created_at || 0,
      status: 'complete',
      parts: [{ type: 'text', text: message.translation, format: 'markdown' }],
      metadata: { translationLang: message.translation_lang, sourceMessageId: message.id },
    }]
  }
  return adapted
}


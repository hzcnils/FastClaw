import { CHAT_WORKSPACE_PROTOCOL_VERSION } from './types'
import { textFromMessage } from './core'
import type {
  ChatMessage, ChatPart, ChatQuestionNavigationItem, ChatStreamEvent, ChatThemeTokens,
  ChatTimelineEntry, ChatTurnComparisonMode, ChatTurnTimelineEntry, NormalizedChatInput,
} from './types'

export interface PendingTurnOptions {
  turnId: string
  userMessageId: string
  assistantMessageId: string
  createdAt?: ChatMessage['createdAt']
}

export interface StreamApplyContext {
  entryId?: string
  messageId?: string
}

export function messagesToTimelineEntries(messages: readonly ChatMessage[]): ChatTimelineEntry[] {
  return messages.map(message => ({
    protocolVersion: CHAT_WORKSPACE_PROTOCOL_VERSION,
    kind: 'message',
    id: `message:${message.id}`,
    message,
  }))
}

export function messagesFromTimeline(entries: readonly ChatTimelineEntry[]): ChatMessage[] {
  return entries.flatMap(entry => {
    if (entry.kind === 'message') return [entry.message]
    if (entry.kind === 'turn') return [entry.user, ...entry.alternatives]
    return []
  })
}

export function lastCompletedAssistantMessageId(entries: readonly ChatTimelineEntry[]): string | undefined {
  const messages = messagesFromTimeline(entries)
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index]
    if (message.role === 'assistant' && message.status !== 'pending' && message.status !== 'streaming') return message.id
  }
  return undefined
}

export function questionItemsFromTimeline(entries: readonly ChatTimelineEntry[]): ChatQuestionNavigationItem[] {
  const result: ChatQuestionNavigationItem[] = []
  for (const entry of entries) {
    let label = ''
    if (entry.kind === 'turn') label = textFromMessage(entry.user)
    else if (entry.kind === 'message' && entry.message.role === 'user') label = textFromMessage(entry.message)
    else if (entry.kind === 'embedded' && typeof entry.metadata?.navigationLabel === 'string') {
      label = entry.metadata.navigationLabel
    }
    label = label.trim()
    if (!label) continue
    result.push({
      id: `question:${entry.id}`,
      entryId: entry.id,
      label: label.length > 80 ? `${label.slice(0, 79)}…` : label,
      index: result.length,
    })
  }
  return result
}

function inputParts(input: NormalizedChatInput): ChatPart[] {
  const parts: ChatPart[] = []
  if (input.text) parts.push({ type: 'text', text: input.text, format: 'plain' })
  for (const attachment of input.attachments || []) {
    const url = attachment.previewUrl || attachment.url
    if (attachment.kind === 'image' && url) {
      parts.push({ type: 'image', src: url, name: attachment.name })
    } else {
      parts.push({
        type: 'attachment', name: attachment.name, url: attachment.url,
        size: attachment.size, mimeType: attachment.mimeType,
      })
    }
  }
  return parts
}

export function createPendingChatTurn(input: NormalizedChatInput, options: PendingTurnOptions): ChatTurnTimelineEntry {
  const createdAt = options.createdAt ?? Date.now()
  return {
    protocolVersion: CHAT_WORKSPACE_PROTOCOL_VERSION,
    kind: 'turn',
    id: options.turnId,
    user: {
      protocolVersion: 1,
      id: options.userMessageId,
      role: 'user',
      createdAt,
      status: 'complete',
      turnId: options.turnId,
      parts: inputParts(input),
      metadata: input.metadata,
    },
    alternatives: [{
      protocolVersion: 1,
      id: options.assistantMessageId,
      role: 'assistant',
      createdAt,
      status: 'pending',
      turnId: options.turnId,
      model: input.modelId ? { id: input.modelId, name: input.modelId } : undefined,
      parts: [],
    }],
    activeId: options.assistantMessageId,
    comparisonMode: 'single',
  }
}

function updatePart(message: ChatMessage, event: ChatStreamEvent): ChatMessage {
  if (event.type === 'message-start') {
    return { ...event.message, turnId: event.message.turnId || message.turnId, status: event.message.status || 'streaming' }
  }
  if (event.type === 'message-complete' && event.message) {
    return { ...event.message, turnId: event.message.turnId || message.turnId, status: 'complete' }
  }

  const parts = [...message.parts]
  if (event.type === 'reasoning-delta' || event.type === 'text-delta') {
    const type = event.type === 'reasoning-delta' ? 'reasoning' : 'text'
    let index = -1
    for (let cursor = parts.length - 1; cursor >= 0; cursor -= 1) {
      if (parts[cursor].type === type) { index = cursor; break }
    }
    if (index >= 0) {
      const current = parts[index]
      if (type === 'reasoning' && current.type === 'reasoning') {
        parts[index] = { ...current, text: current.text + event.delta, status: 'streaming' }
      } else if (type === 'text' && current.type === 'text') {
        parts[index] = { ...current, text: current.text + event.delta, format: current.format || 'markdown' }
      }
    } else {
      parts.push(type === 'reasoning'
        ? { type: 'reasoning', text: event.delta, status: 'streaming' }
        : { type: 'text', text: event.delta, format: 'markdown' })
    }
    return { ...message, status: 'streaming', parts }
  }
  if (event.type === 'part') return { ...message, status: 'streaming', parts: [...parts, event.part] }
  if (event.type === 'attachments-ready') return { ...message, parts: [...parts, ...event.attachments] }
  if (event.type === 'message-complete') {
    return {
      ...message,
      status: 'complete',
      parts: parts.map(part => part.type === 'reasoning' ? { ...part, status: 'complete' } : part),
    }
  }
  if (event.type === 'error') {
    return {
      ...message,
      status: 'error',
      parts: [...parts, { type: 'notice', text: event.error.message, tone: 'error' }],
      metadata: { ...message.metadata, streamError: event.error },
    }
  }
  return message
}

export function applyChatStreamEvent(
  entries: readonly ChatTimelineEntry[],
  event: ChatStreamEvent,
  context: StreamApplyContext = {},
): ChatTimelineEntry[] {
  const eventMessageId = 'messageId' in event ? event.messageId : undefined
  const targetId = eventMessageId || context.messageId
  let applied = false

  const updateMessage = (message: ChatMessage, entryId: string, fallback: boolean) => {
    const matches = targetId ? message.id === targetId : context.entryId === entryId && fallback
    if (!matches) return message
    applied = true
    return updatePart(message, event)
  }

  const result = entries.map(entry => {
    if (entry.kind === 'message') {
      const message = updateMessage(entry.message, entry.id, entry.message.role === 'assistant')
      return message === entry.message ? entry : { ...entry, message }
    }
    if (entry.kind === 'turn') {
      const fallbackId = entry.activeId || entry.alternatives[entry.alternatives.length - 1]?.id
      const alternatives = entry.alternatives.map(message => updateMessage(message, entry.id, message.id === fallbackId))
      return alternatives.every((message, index) => message === entry.alternatives[index]) ? entry : { ...entry, alternatives }
    }
    return entry
  })
  return applied ? result : [...entries]
}

export function updateTimelineVariant(
  entries: readonly ChatTimelineEntry[],
  turnId: string,
  activeId: string,
): ChatTimelineEntry[] {
  return entries.map(entry => entry.kind === 'turn' && entry.id === turnId && entry.alternatives.some(item => item.id === activeId)
    ? { ...entry, activeId }
    : entry)
}

export function updateTimelineComparisonMode(
  entries: readonly ChatTimelineEntry[],
  turnId: string,
  mode: ChatTurnComparisonMode,
): ChatTimelineEntry[] {
  return entries.map(entry => entry.kind === 'turn' && entry.id === turnId ? { ...entry, comparisonMode: mode } : entry)
}

const themeTokenMap: Record<keyof ChatThemeTokens, string> = {
  background: '--jcr-bg', foreground: '--jcr-fg', muted: '--jcr-muted', border: '--jcr-border',
  primary: '--jcr-primary', userBackground: '--jcr-user-bg', assistantBackground: '--jcr-assistant-bg',
  danger: '--jcr-danger', codeBackground: '--jcr-code-bg', surface: '--jcr-surface',
  subtleBackground: '--jcr-subtle-bg', radius: '--jcr-radius', fontSize: '--jcr-font-size',
  gap: '--jcr-gap', bubbleMaxWidth: '--jcr-bubble-max-width', auxiliaryWidth: '--jcr-auxiliary-width',
}

export function createChatThemeStyle(tokens: ChatThemeTokens = {}): Record<string, string> {
  const style: Record<string, string> = {}
  for (const [key, value] of Object.entries(tokens)) {
    if (!value) continue
    const variable = themeTokenMap[key as keyof ChatThemeTokens]
    if (variable) style[variable] = value
    else if (key.startsWith('--')) style[key] = value
  }
  return style
}

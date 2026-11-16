import type { ChatEmbeddedTimelineEntry, ChatMessage, ChatTimelineEntry, ChatTurnComparisonMode } from 'chat-renderer'
import type { Message } from '@/types'
import { adaptMessage, type MessageAdapterOptions } from './messageAdapter'

interface HostMessageGroup {
  groupId: string | null
  messages: Message[]
  startIndex: number
}

export interface TimelineAdapterOptions extends MessageAdapterOptions {
  comparisonMode?: ChatTurnComparisonMode | ((groupId: string, alternatives: number) => ChatTurnComparisonMode)
  contextDividerLabel?: string
  adaptMessage?: (message: Message, options: MessageAdapterOptions) => ChatMessage
}

export function groupHostMessages(messages: readonly Message[]): HostMessageGroup[] {
  const groups: HostMessageGroup[] = []
  messages.forEach((message, startIndex) => {
    const groupId = message.group_id || null
    const previous = groups.at(-1)
    if (groupId && previous?.groupId === groupId) previous.messages.push(message)
    else groups.push({ groupId, messages: [message], startIndex })
  })
  return groups
}

function allContextCleared(group: HostMessageGroup): boolean {
  return group.messages.length > 0 && group.messages.every(message => Boolean(message.context_cleared))
}

function contextDivider(group: HostMessageGroup, index: number, label: string): ChatEmbeddedTimelineEntry {
  return {
    kind: 'embedded',
    id: `context-divider:${group.groupId || group.messages.at(-1)?.id || index}`,
    renderer: 'fastclaw-context-divider',
    data: { label },
    status: 'complete',
  }
}

export function adaptMessagesToTimeline(messages: readonly Message[], options: TimelineAdapterOptions = {}): ChatTimelineEntry[] {
  const adapt = options.adaptMessage || adaptMessage
  const groups = groupHostMessages(messages)
  const timeline: ChatTimelineEntry[] = []
  groups.forEach((group, groupIndex) => {
    const user = group.messages.find(message => message.role === 'user')
    const alternatives = group.messages.filter(message => message.role === 'assistant')
    if (group.groupId && user && alternatives.length) {
      const requestedMode = typeof options.comparisonMode === 'function'
        ? options.comparisonMode(group.groupId, alternatives.length)
        : options.comparisonMode
      timeline.push({
        kind: 'turn',
        id: `turn:${group.groupId}`,
        user: adapt(user, options),
        alternatives: alternatives.map(message => adapt(message, options)),
        activeId: alternatives[0]?.id,
        comparisonMode: requestedMode || (alternatives.length > 1 ? 'tabs' : 'single'),
        metadata: { groupId: group.groupId, startIndex: group.startIndex },
      })
      for (const message of group.messages) {
        if (message !== user && message.role !== 'assistant') {
          timeline.push({ kind: 'message', id: `message:${message.id}`, message: adapt(message, options) })
        }
      }
    } else {
      for (const message of group.messages) {
        timeline.push({ kind: 'message', id: `message:${message.id}`, message: adapt(message, options) })
      }
    }
    const next = groups[groupIndex + 1]
    if (allContextCleared(group) && (!next || !allContextCleared(next))) {
      timeline.push(contextDivider(group, groupIndex, options.contextDividerLabel || '上下文已清除'))
    }
  })
  return timeline
}

type CachedHostMessage = {
  source: Message
  adapted: ChatMessage
  content: Message['content']
  status: Message['status']
  think_content: Message['think_content']
  stream_phase: Message['stream_phase']
  isThinking: Message['isThinking']
  planRuntime: Message['planRuntime']
}

function hostMessageUnchanged(cached: CachedHostMessage | undefined, source: Message): boolean {
  return !!cached
    && cached.source === source
    && cached.content === source.content
    && cached.status === source.status
    && cached.think_content === source.think_content
    && cached.stream_phase === source.stream_phase
    && cached.isThinking === source.isThinking
    && cached.planRuntime === source.planRuntime
}

/** Reuses public message and entry objects when the corresponding host message did not change. */
export function createIncrementalTimelineAdapter() {
  let messageCache = new Map<string, CachedHostMessage>()
  let entryCache = new Map<string, ChatTimelineEntry>()
  return (messages: readonly Message[], options: TimelineAdapterOptions = {}): ChatTimelineEntry[] => {
    const nextMessageCache = new Map<string, CachedHostMessage>()
    const cachedAdapt = (source: Message, adapterOptions: MessageAdapterOptions) => {
      const cached = messageCache.get(source.id)
      const adapted = hostMessageUnchanged(cached, source) ? cached.adapted : adaptMessage(source, adapterOptions)
      nextMessageCache.set(source.id, {
        source,
        adapted,
        content: source.content,
        status: source.status,
        think_content: source.think_content,
        stream_phase: source.stream_phase,
        isThinking: source.isThinking,
        planRuntime: source.planRuntime,
      })
      return adapted
    }
    const fresh = adaptMessagesToTimeline(messages, { ...options, adaptMessage: cachedAdapt })
    const nextEntryCache = new Map<string, ChatTimelineEntry>()
    const stable = fresh.map((entry) => {
      const previous = entryCache.get(entry.id)
      let value = entry
      if (previous?.kind === 'message' && entry.kind === 'message' && previous.message === entry.message) value = previous
      else if (previous?.kind === 'turn' && entry.kind === 'turn'
        && previous.user === entry.user
        && previous.activeId === entry.activeId
        && previous.comparisonMode === entry.comparisonMode
        && previous.alternatives.length === entry.alternatives.length
        && previous.alternatives.every((message, index) => message === entry.alternatives[index])) value = previous
      else if (previous?.kind === 'embedded' && entry.kind === 'embedded' && previous.renderer === entry.renderer) value = previous
      nextEntryCache.set(value.id, value)
      return value
    })
    messageCache = nextMessageCache
    entryCache = nextEntryCache
    return stable
  }
}

import { MESSAGE_ACTION_DIVIDER } from './types'
import type {
  ChatMessage, MessageActionContext, MessageActionName, MessageActionsConfig, MessageActionResolverEntry,
  ResolvedMessageAction, ActionResolver, ToolCallPart, ChatComposerAction, ChatContextItem,
} from './types'

export interface ChatTurn { id: string; messages: ChatMessage[] }

export function resolveComposerActions(actions: readonly ChatComposerAction[]): ChatComposerAction[] {
  return actions
    .map((action, index) => ({ action, index }))
    .filter(({ action }) => action.visible !== false)
    .sort((a, b) => (a.action.order ?? 0) - (b.action.order ?? 0) || a.index - b.index)
    .map(({ action }) => action)
}

export function visibleContextItems(items: readonly ChatContextItem[]): ChatContextItem[] {
  return items.filter(item => item.visible !== false)
}

export function groupMessagesIntoTurns(messages: readonly ChatMessage[]): ChatTurn[] {
  const turns: ChatTurn[] = []
  let current: ChatTurn | undefined
  for (const message of messages) {
    const explicit = message.turnId
    if (!current || (message.role === 'user' && current.messages.length) || (explicit && explicit !== current.id)) {
      current = { id: explicit || `turn-${message.id}`, messages: [] }
      turns.push(current)
    }
    current.messages.push(message)
  }
  return turns
}

export function isToolComplete(part: ToolCallPart): boolean {
  return part.status === 'complete' || part.status === 'error' || part.output !== undefined || !!part.error
}

export function groupConsecutiveTools(parts: readonly ToolCallPart[]): ToolCallPart[][] {
  const groups: ToolCallPart[][] = []
  for (const part of parts) {
    const previous = groups[groups.length - 1]
    if (previous && isToolComplete(part) === isToolComplete(previous[0])) previous.push(part)
    else groups.push([part])
  }
  return groups
}

const defaultsByRole: Record<string, MessageActionName[]> = {
  user: ['copy', 'edit', 'resend'],
  assistant: ['copy', 'regenerate', 'like', 'dislike'],
  system: [],
  tool: [],
}

const rolesByAction: Partial<Record<MessageActionName, Set<ChatMessage['role']>>> = {
  edit: new Set(['user', 'assistant']),
  resend: new Set(['user']),
  regenerate: new Set(['assistant']),
  like: new Set(['assistant']),
  dislike: new Set(['assistant']),
  'edit-as-doc': new Set(['assistant']),
  'switch-model': new Set(['assistant']),
  'trace-source': new Set(['assistant']),
  feedback: new Set(['assistant']),
}

const labels: Record<MessageActionName, string> = {
  copy: 'copy', edit: 'edit', resend: 'resend', regenerate: 'regenerate', quote: 'quote', tts: 'tts',
  translate: 'translate', favorite: 'favorite', export: 'export', feedback: 'feedback', like: 'like', dislike: 'dislike',
  'edit-as-doc': 'edit-as-doc', 'switch-model': 'switch-model', 'trace-source': 'trace-source',
  branch: 'branch', delete: 'delete', custom: 'custom',
}

const stableIdActions = new Set<MessageActionName>(['edit', 'resend', 'regenerate', 'favorite', 'feedback', 'like', 'dislike', 'branch', 'delete'])
const blockedWhileGenerating = new Set<MessageActionName>(['edit', 'delete', 'regenerate', 'export'])

export function resolveMessageActions(
  message: ChatMessage,
  context: MessageActionContext,
  showActions = true,
  config: MessageActionsConfig = {},
  resolver?: ActionResolver,
): ResolvedMessageAction[] {
  if (!showActions || message.status === 'error') return []
  const baseNames = defaultsByRole[message.role] || []
  const configuredNames = (Object.keys(config) as MessageActionName[])
    .filter(name => !rolesByAction[name] || rolesByAction[name]?.has(message.role))
  const names = [...new Set([...baseNames, ...configuredNames])]
  let resolved = names.map((name, index): ResolvedMessageAction => {
    const raw = config[name]
    const detail = typeof raw === 'object' ? raw : {}
    const enabled = raw !== false && detail.enabled !== false
    const invalidId = stableIdActions.has(name) && !context.hasStableId
    const invalidGeneration = context.isGenerating && blockedWhileGenerating.has(name)
    const invalidRegenerate = name === 'regenerate' && !context.isLastCompletedAssistantTurn
    return {
      ...detail,
      name,
      actionId: detail.actionId || name,
      label: detail.label || labels[name],
      icon: detail.icon,
      order: detail.order ?? index,
      visible: enabled && detail.visible !== false && !invalidId && !invalidGeneration && !invalidRegenerate,
      disabled: detail.disabled === true || detail.loading === true,
    }
  }).filter(action => action.visible)

  if (resolver) {
    const override = resolver(message, context)
    if (override.length && typeof override[0] === 'string') {
      const allowed = new Set<MessageActionName>()
      const divided = new Set<MessageActionName>()
      const resolverOrder = new Map<MessageActionName, number>()
      let dividerBeforeNext = false
      for (const entry of override as MessageActionResolverEntry[]) {
        if (entry === MESSAGE_ACTION_DIVIDER) {
          dividerBeforeNext = true
          continue
        }
        const name = entry as MessageActionName
        allowed.add(name)
        if (!resolverOrder.has(name)) resolverOrder.set(name, resolverOrder.size)
        if (dividerBeforeNext) divided.add(name)
        dividerBeforeNext = false
      }
      resolved = resolved
        .filter(action => allowed.has(action.name))
        .map(action => ({
          ...action,
          order: resolverOrder.get(action.name) ?? action.order,
          ...(divided.has(action.name) ? { dividerBefore: true } : {}),
        }))
    } else {
      resolved = (override as ResolvedMessageAction[]).filter(action => action.visible !== false)
    }
  }
  return resolved.sort((a, b) => a.order - b.order)
}

/** Calculates the largest ordered action prefix that fits beside an overflow button. */
export function calculateDirectActionCount(actions: readonly ResolvedMessageAction[], width: number, fallback = 5): number {
  if (!actions.length) return 0
  if (!width) return Math.min(fallback, actions.length)
  let used = 0
  let count = 0
  for (let index = 0; index < actions.length; index++) {
    const action = actions[index]
    const previous = actions[index - 1]
    const divider = index > 0 && (action.dividerBefore || action.name === 'like' || action.name === 'dislike')
      && previous?.name !== 'like' && previous?.name !== 'dislike'
    const itemWidth = 28 + (divider ? 9 : 0) + (count ? 2 : 0)
    const reserveMore = index < actions.length - 1 ? 32 : 0
    if (count && used + itemWidth + reserveMore > width) break
    used += itemWidth
    count++
  }
  return Math.max(1, count)
}

export function textFromMessage(message: ChatMessage): string {
  return message.parts
    .filter((part): part is Extract<typeof part, { type: 'text' }> => part.type === 'text')
    .map(part => part.text)
    .join('\n\n')
}

export function safeUrl(value: string, kind: 'link' | 'image' = 'link'): string | null {
  const raw = value.trim()
  if (!raw) return null
  if (kind === 'image' && /^data:image\/(png|jpe?g|gif|webp);base64,[a-z0-9+/=\s]+$/i.test(raw)) {
    return raw.length <= 14_000_000 ? raw : null
  }
  if (raw.startsWith('#') || raw.startsWith('/')) return raw
  try {
    const url = new URL(raw, 'https://chat-renderer.invalid')
    const allowed = kind === 'image'
      ? new Set(['http:', 'https:', 'blob:'])
      : new Set(['http:', 'https:', 'mailto:', 'tel:'])
    return allowed.has(url.protocol) ? raw : null
  } catch {
    return null
  }
}

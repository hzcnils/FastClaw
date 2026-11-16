import type {
  ActionResolver,
  ChatWorkspaceActionEvent,
  MessageActionName,
  MessageActionsConfig,
  ResolvedMessageAction,
} from 'chat-renderer'

export const FASTCLAW_MESSAGE_ACTIONS: MessageActionsConfig = {
  copy: true,
  edit: true,
  resend: true,
  regenerate: true,
  quote: true,
  tts: true,
  translate: { menuItems: [
    { id: 'zh', label: '中文' }, { id: 'zh-TW', label: '繁體中文' }, { id: 'en', label: 'English' },
    { id: 'ja', label: '日本語' }, { id: 'ko', label: '한국어' }, { id: 'fr', label: 'Français' }, { id: 'es', label: 'Español' },
  ] },
  favorite: true,
  'edit-as-doc': true,
  'switch-model': true,
  'trace-source': { menuItems: [
    { id: 'local', label: '本地知识库' }, { id: 'graph', label: '知识图谱' }, { id: 'file', label: '文件' },
    { id: 'result', label: '结果' }, { id: 'analysis', label: '分析' },
  ] },
  export: { menuItems: [
    { id: 'md', label: '导出为Markdown' }, { id: 'docx', label: '导出为DOCX' }, { id: 'pdf', label: '导出为PDF' },
  ] },
  feedback: true,
  like: true,
  dislike: true,
  branch: true,
  delete: { danger: true },
}

function assistantActionNames(
  message: Parameters<ActionResolver>[0],
  context: Parameters<ActionResolver>[1],
): MessageActionName[] {
  const actions: MessageActionName[] = ['copy', 'edit', 'translate', 'favorite', 'edit-as-doc']
  if (context.isLastCompletedAssistantTurn) actions.splice(1, 0, 'regenerate')
  if (message.turnId) actions.push('switch-model')
  actions.push('quote', 'tts')
  if (message.metadata?.traceSource) actions.push('trace-source')
  actions.push('export', 'like', 'dislike', 'branch', 'delete')
  return actions
}

function toResolvedActions(
  names: MessageActionName[],
  options?: { translating?: boolean; translatingLabel?: string },
): ResolvedMessageAction[] {
  const translating = Boolean(options?.translating)
  const translatingLabel = options?.translatingLabel || '翻译中...'
  return names.map((name, index) => {
    const raw = FASTCLAW_MESSAGE_ACTIONS[name]
    const detail = typeof raw === 'object' && raw ? { ...raw } : {}
    const isTranslating = translating && name === 'translate'
    return {
      ...detail,
      name,
      actionId: detail.actionId || name,
      label: isTranslating ? translatingLabel : (detail.label || name),
      tooltip: isTranslating ? translatingLabel : detail.tooltip,
      order: detail.order ?? index,
      visible: true,
      disabled: isTranslating || detail.disabled === true,
      loading: isTranslating || detail.loading === true,
    }
  })
}

/** 可注入翻译中文案（i18n）；默认与旧版 MessageBubble 一致 */
export function createResolveFastClawMessageActions(options?: {
  translatingLabel?: () => string
}): ActionResolver {
  return (message, context) => {
    if (message.metadata?.vendorConfig) return []
    if (message.status === 'error') return []
    if (message.role === 'user') {
      return context.isGenerating ? ['copy'] : ['copy', 'edit', 'resend']
    }
    if (message.role !== 'assistant') return ['copy']
    if (message.status === 'pending' || message.status === 'streaming') return ['copy']
    const names = assistantActionNames(message, context)
    if (message.metadata?.translating) {
      return toResolvedActions(names, {
        translating: true,
        translatingLabel: options?.translatingLabel?.() || '翻译中...',
      })
    }
    return names
  }
}

export const resolveFastClawMessageActions: ActionResolver = createResolveFastClawMessageActions()

export interface FastClawActionHandlers {
  [action: string]: (event: ChatWorkspaceActionEvent) => void | Promise<void>
}

export async function routeFastClawAction(
  event: ChatWorkspaceActionEvent,
  handlers: FastClawActionHandlers,
): Promise<boolean> {
  const handler = handlers[event.action]
  if (!handler) return false
  await handler(event)
  return true
}

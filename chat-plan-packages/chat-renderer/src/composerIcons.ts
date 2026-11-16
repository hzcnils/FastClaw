export type ChatComposerIconName = 'image' | 'file' | 'task' | 'voice' | 'clear' | 'thinking' | 'planning'
  | 'context-clear' | 'conversation-clear' | 'export' | 'send' | 'stop' | 'more'

const aliases: Record<string, ChatComposerIconName> = {
  image: 'image', 'attach-image': 'image', 'upload-image': 'image', file: 'file', 'attach-file': 'file',
  'upload-file': 'file', task: 'task', 'attach-task': 'task', 'pick-task': 'task', voice: 'voice', mic: 'voice',
  clear: 'clear', 'clear-input': 'clear', thinking: 'thinking', think: 'thinking', 'toggle-thinking': 'thinking',
  planning: 'planning', plan: 'planning', 'toggle-planning': 'planning', 'task-plan': 'planning',
  'clear-context': 'context-clear', 'delete-context': 'context-clear',
  'clear-conversation': 'conversation-clear', 'delete-conversation': 'conversation-clear',
  export: 'export', 'export-conversation': 'export', download: 'export',
  send: 'send', submit: 'send', stop: 'stop', abort: 'stop', cancel: 'stop',
}

export function resolveComposerIconName(actionId: string): ChatComposerIconName {
  return aliases[actionId.trim().toLowerCase().replace(/[\s_]+/g, '-')] || 'more'
}

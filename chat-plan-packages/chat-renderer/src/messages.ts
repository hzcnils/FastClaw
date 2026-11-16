import type { ChatRendererMessages } from './types'

/** Supported built-in UI locales. Any other locale can be supplied with `labels`. */
export type ChatRendererLocale = 'zh-CN' | 'en-US'

export const defaultMessages: ChatRendererMessages = {
  empty: '暂无消息', loading: '正在加载消息…', generating: '正在生成…',
  stalled: '响应可能已停滞。', recover: '停止并恢复', loadOlder: '加载更早消息',
  loadAll: '加载全部', copy: '复制', copied: '已复制', edit: '编辑', resend: '重新发送',
  regenerate: '重新生成', quote: '引用', tts: '朗读', translate: '翻译',
  favorite: '收藏', 'edit-as-doc': '作为文档编辑', 'switch-model': '切换模型',
  'trace-source': '追溯来源', export: '导出', feedback: '反馈', like: '赞', dislike: '踩', branch: '分支',
  delete: '删除', confirm: '确认', cancel: '取消', reasoning: '推理过程',
  toolInput: '输入', toolOutput: '输出', unknownContent: '不支持的内容',
  showFullContent: '查看完整内容', newMessages: '新消息', scrollToBottom: '滚动到底部', more: '更多',
  questionDirectory: '问题目录', openQuestionDirectory: '打开问题目录', closeQuestionDirectory: '关闭问题目录',
  modelVariants: '模型回答', tabs: '标签页', split: '对比', missingRenderer: '渲染器不可用',
  close: '关闭', attachmentIdle: '就绪', attachmentUploading: '正在上传', attachmentProcessing: '正在处理',
  attachmentComplete: '完成', attachmentError: '失败', attachmentReference: '引用此文档',
}

export const englishMessages: ChatRendererMessages = {
  empty: 'No messages yet', loading: 'Loading messages…', generating: 'Generating…',
  stalled: 'The response may be stalled.', recover: 'Stop and recover', loadOlder: 'Load older messages',
  loadAll: 'Load all', copy: 'Copy', copied: 'Copied', edit: 'Edit', resend: 'Resend',
  regenerate: 'Regenerate', quote: 'Quote', tts: 'Read aloud', translate: 'Translate',
  favorite: 'Favorite', 'edit-as-doc': 'Edit as document', 'switch-model': 'Switch model',
  'trace-source': 'Trace source', export: 'Export', feedback: 'Feedback', like: 'Like', dislike: 'Dislike', branch: 'Branch',
  delete: 'Delete', confirm: 'Confirm', cancel: 'Cancel', reasoning: 'Reasoning',
  toolInput: 'Input', toolOutput: 'Output', unknownContent: 'Unsupported content',
  showFullContent: 'Show full content', newMessages: 'New messages', scrollToBottom: 'Scroll to bottom', more: 'More',
  questionDirectory: 'Question directory', openQuestionDirectory: 'Open question directory', closeQuestionDirectory: 'Close question directory',
  modelVariants: 'Model answers', tabs: 'Tabs', split: 'Compare', missingRenderer: 'Renderer unavailable',
  close: 'Close', attachmentIdle: 'Ready', attachmentUploading: 'Uploading', attachmentProcessing: 'Processing',
  attachmentComplete: 'Complete', attachmentError: 'Failed', attachmentReference: 'Reference this document',
}

export const chatRendererMessages: Record<ChatRendererLocale, ChatRendererMessages> = {
  'zh-CN': defaultMessages,
  'en-US': englishMessages,
}

/** Resolves built-in text first, then lets a host language pack override any key. */
export function resolveChatRendererMessages(locale?: string, labels?: Partial<ChatRendererMessages>): ChatRendererMessages {
  return { ...(locale?.toLowerCase().startsWith('en') ? englishMessages : defaultMessages), ...labels }
}

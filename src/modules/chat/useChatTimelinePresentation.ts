import { computed } from 'vue'
import type { ChatEmbeddedTimelineEntry, ChatMessage, ChatTimelineEntry } from 'chat-renderer'
import type { Message } from '@/types'
import { createIncrementalTimelineAdapter } from '@/integrations/chat-plan/timelineAdapter'

// Presentation intentionally excludes provider credentials, form values and execution commands.
export interface ProviderTimelinePanel {
  id: string
  createdAt: number
  modelId: string
  userMessage?: ChatMessage
}
export interface ChatTimelinePresentationOptions {
  messages: () => readonly Message[]
  fastPlanEntries: () => ChatEmbeddedTimelineEntry[]
  taskPlanEnabled: () => boolean
  providerPanel: () => ProviderTimelinePanel | null
  modelName: (id: string | null) => string | undefined
  contextDividerLabel: () => string
}

/** One incremental adapter/cache per workspace; no Store, routing or transport ownership. */
export function useChatTimelinePresentation(options: ChatTimelinePresentationOptions) {
  const adaptOrdinaryTimeline = createIncrementalTimelineAdapter();
  const ordinaryChatTimeline = computed(() => adaptOrdinaryTimeline(options.messages(), {
    modelName: options.modelName,
    contextDividerLabel: options.contextDividerLabel(),
  }));
  // 规划模式优先展示规划时间线；若本会话仍有普通消息，一并保留，避免历史被盖住。
  // 退出规划后只展示普通会话。这里只隔离展示状态，不删除宿主消息或已持久化规划数据。
  const providerCompletionTimeline = computed<ChatTimelineEntry[]>(() => {
    const panel = options.providerPanel();
    if (!panel) return [];
    const timeline: ChatTimelineEntry[] = panel.userMessage
      ? [{ kind: 'message', id: panel.userMessage.id, message: panel.userMessage }]
      : [];
    return [...timeline, {
      kind: 'message', id: panel.id,
      message: {
        protocolVersion: 1, id: panel.id, role: 'assistant', createdAt: panel.createdAt, status: 'complete',
        model: { id: panel.modelId, name: options.modelName(panel.modelId) || panel.modelId },
        parts: [{ type: 'custom', kind: 'fastclaw-vendor-config', data: {} }],
        metadata: { vendorConfig: true },
      },
    }];
  });
  let cachedChatPlanTimeline: ChatTimelineEntry[] = [];
  let cachedOrdinaryTimeline: ChatTimelineEntry[] | null = null;
  let cachedFastPlanTimeline: ChatEmbeddedTimelineEntry[] | null = null;
  let cachedProviderTimeline: ChatTimelineEntry[] | null = null;
  let cachedTaskPlanEnabled: boolean | null = null;
  const chatPlanTimeline = computed<ChatTimelineEntry[]>(() => {
    const ordinary = ordinaryChatTimeline.value;
    const fastPlan = options.fastPlanEntries();
    const provider = providerCompletionTimeline.value;
    const taskPlanEnabled = options.taskPlanEnabled();
    if (
      cachedOrdinaryTimeline === ordinary
      && cachedFastPlanTimeline === fastPlan
      && cachedProviderTimeline === provider
      && cachedTaskPlanEnabled === taskPlanEnabled
    ) return cachedChatPlanTimeline;

    cachedOrdinaryTimeline = ordinary;
    cachedFastPlanTimeline = fastPlan;
    cachedProviderTimeline = provider;
    cachedTaskPlanEnabled = taskPlanEnabled;
    if (!taskPlanEnabled) {
      cachedChatPlanTimeline = [...ordinary, ...provider];
      return cachedChatPlanTimeline;
    }
    if (ordinary.length) {
      cachedChatPlanTimeline = [...ordinary, ...fastPlan, ...provider];
      return cachedChatPlanTimeline;
    }
    cachedChatPlanTimeline = [...fastPlan, ...provider];
    return cachedChatPlanTimeline;
  });


  return { ordinaryChatTimeline, providerCompletionTimeline, chatPlanTimeline }
}

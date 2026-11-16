<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { ChatConversation } from 'chat-renderer'
import type { ChatEmbeddedNavigationItem } from 'chat-renderer'
import { createFastPlanClient, normalizeFastPlanError, uploadFastPlanFile } from 'planflow-runtime'
import FastPlanConversation from './FastPlanConversation.vue'
import type { FastPlanInteractionCommand } from './interaction'
import type {
  FastPlanTimelineCommand,
  FastPlanTimelineEntryData,
} from './timelinePlugin'

const props = defineProps<{
  entry: { id: string; status?: string }
  data: FastPlanTimelineEntryData
  status?: string
  // Keep the SFC macro self-contained for hosts whose compiler cannot resolve
  // imported interfaces while compiling a linked workspace package.
  config: Record<string, any>
}>()
const UntypedChatConversation = ChatConversation as any
const emit = defineEmits<{ (event: 'action', value: Record<string, unknown>): void }>()

const conversationRef = ref<any>(null)
const localSessionId = ref(String(props.data.sessionId || ''))
const pendingPrompt = ref('')
const pendingError = ref('')
const uploadProgress = ref<Record<string, number>>({})
const runtimeOwnerId = `${props.entry.id}:${globalThis.crypto?.randomUUID?.() || Math.random().toString(36).slice(2)}`
let handledCommandId = ''
let disposed = false
/** 宿主点停止后，打断尚在 createSession / waitUntilConnected / sendChat 中的 send */
let abortRequested = false

const effectiveModel = computed(() => props.data.model || props.config.model || '')
const effectiveExecutionMode = computed(() => props.data.executionMode || props.config.executionMode || 'manual')
const effectiveConfirmationPolicy = computed(() => props.data.confirmationPolicy || props.config.confirmationPolicy || 'strict')
const effectiveConnectionMode = computed(() => props.data.connectionMode || (props.data.command ? 'live' : 'history'))
const isComposerEntry = computed(() => props.config.composerMode !== 'host'
  || (!!props.config.composerEntryId && props.config.composerEntryId === props.entry.id))
const effectiveComposerMode = computed(() => props.config.composerMode === 'host'
  && isComposerEntry.value
  ? 'host'
  : 'internal')
// In a host-owned composer only the selected (normally latest) timeline entry
// contributes its task list. History entries keep their messages in the
// timeline without duplicating a scroll-bound task list.
const effectiveShowTaskList = computed(() => props.config.composerMode !== 'host' || isComposerEntry.value)
const avatarUrl = computed(() => String(props.config.avatar || ''))
const themeTokens = computed(() => ({
  background: 'transparent',
  assistantBackground: 'transparent',
  ...props.config.themeTokens,
  '--jcr-panel-padding': '0',
}))
const displayedPendingPrompt = computed(() => pendingPrompt.value || String(props.data.pendingPrompt || '').trim())
const displayedPendingError = computed(() => pendingError.value || String(props.data.preflightError || '').trim())
const displayedPendingAttachments = computed(() => props.data.pendingAttachments || props.data.command?.attachments || [])
const historyExpanded = ref(false)
const historyDeferred = computed(() => props.data.deferLoad === true
  && effectiveConnectionMode.value === 'history'
  && !isComposerEntry.value)
const shouldMountConversation = computed(() => !historyDeferred.value || historyExpanded.value)
const historySummary = computed(() => String(props.data.prompt || '历史任务规划').trim())
const initialFeedback = computed(() => {
  const map = (props.data as { feedbackByMessageId?: Record<string, number> }).feedbackByMessageId
  return map && typeof map === 'object' ? map : {}
})
const managedRuntime = computed(() => {
  const manager = props.config.sessionManager
  const sessionId = localSessionId.value
  if (!manager || !sessionId || (effectiveConnectionMode.value !== 'live' && props.data.commandOwner !== 'host')) return undefined
  return manager.getOrCreate(sessionId, {
    apiBase: props.config.apiBase,
    wsBase: props.config.wsBase,
    access: props.config.access,
    model: effectiveModel.value,
    executionMode: effectiveExecutionMode.value,
    confirmationPolicy: effectiveConfirmationPolicy.value,
    maxReconnectAttempts: props.config.maxReconnectAttempts,
    baseReconnectDelayMs: props.config.baseReconnectDelayMs,
    retainConnectionOnSessionSwitch: true,
    persistUserMessages: false,
  })
})
const pendingMessages = computed(() => displayedPendingPrompt.value ? [{
  protocolVersion: 1,
  id: `pending-${props.entry.id}`,
  role: 'user',
  createdAt: Date.now(),
  status: displayedPendingError.value ? 'failed' : 'pending',
  parts: [
    { type: 'text', text: displayedPendingPrompt.value, format: 'plain' },
    ...(displayedPendingAttachments.value.map(file => ({
      type: 'attachment',
      name: String(file.name || '附件'),
      size: Number(file.size) || undefined,
      mimeType: String(file.mimeType || file.type || ''),
    }))),
  ],
}] : [])

const interactionDispatcher = computed(() => {
  const manager = props.config.sessionManager
  if (!managedRuntime.value || !manager?.executeEntryControl) return undefined
  // Capture this renderer's identity, never whichever entry is active after an await.
  const sessionId = localSessionId.value
  const entryId = props.entry.id
  return (command: FastPlanInteractionCommand): Promise<boolean> => disposed
    ? Promise.resolve(false)
    : manager.executeEntryControl(sessionId, entryId, command)
})

function action(type: string, details: Record<string, unknown> = {}) {
  emit('action', { type, entryId: props.entry.id, sessionId: localSessionId.value, ...details })
}

function normalizeError(value: unknown) {
  const error = normalizeFastPlanError(value)
  return {
    message: error.message,
    code: error.code,
    terminal: error.terminal === true,
    details: error.details ?? error.cause ?? value,
  }
}

async function createSession(title: string) {
  const session = await createFastPlanClient({ apiBase: props.config.apiBase, access: props.config.access }).createSession(title.slice(0, 50))
  const id = String(session?.id || session?.session_id || '')
  if (!id) throw new Error('FastPlan 创建会话成功但未返回 session id')
  localSessionId.value = id
  action('session-created', { session, prompt: title })
  await nextTick()
  return id
}

async function waitUntilConnected(timeout = 10000) {
  const startedAt = Date.now()
  while (!disposed) {
    const runtime = conversationRef.value?.runtime
    if (runtime?.connected?.value && !runtime?.loading?.value) return
    if (Date.now() - startedAt >= timeout) throw new Error('FastPlan WebSocket 连接超时')
    await new Promise(resolve => setTimeout(resolve, 50))
  }
  throw new Error('FastPlan 时间线节点已卸载')
}

async function uploadAttachments(command: FastPlanTimelineCommand) {
  const result: Record<string, unknown>[] = []
  for (const item of command.attachments || []) {
    if (item.path || item.url) {
      result.push(item)
      continue
    }
    const metadata = item.metadata && typeof item.metadata === 'object'
      ? item.metadata as Record<string, unknown>
      : {}
    const file = item.file || item.rawFile || metadata.rawFile
    if (!(file instanceof File)) continue
    const id = String(item.id || file.name)
    const upload = uploadFastPlanFile(file, {
      access: props.config.access,
      apiBase: props.config.uploadBase || props.config.apiBase,
      onProgress: progress => {
        uploadProgress.value = { ...uploadProgress.value, [id]: progress }
        action('attachment-progress', { attachmentId: id, progress })
      },
    })
    const uploaded = await upload.promise
    result.push({
      id,
      name: item.name || file.name,
      filename: item.name || file.name,
      size: item.size || file.size,
      mimeType: item.mimeType || file.type,
      path: uploaded.path,
    })
    action('attachment-complete', { attachmentId: id, path: uploaded.path })
  }
  return result
}

async function runCommand(command?: FastPlanTimelineCommand) {
  if (props.data.commandOwner === 'host' && (command?.type === 'send' || command?.type === 'stop')) return
  if (!command?.id || command.id === handledCommandId) return
  handledCommandId = command.id
  pendingError.value = ''

  if (command.type === 'stop') {
    abortRequested = true
    pendingPrompt.value = ''
    const instance = conversationRef.value
    try {
      if (instance) {
        const state = instance.state
        // 意图理解 / 匹配分析 / 生成任务树阶段走 cancel_plan；树已在执行时走 cancel
        const accepted = state?.isPreviewing || state?.isPlanning || !state?.currentTreeId
          ? await instance.cancelPlan?.() : await instance.cancelExecution?.()
        if (accepted === false) return
      }
    } catch (error) {
      if (!disposed) action('error', { error: normalizeError(error) })
      return
    }
    action('busy-change', { busy: false, command: command.type })
    action('execution-completed', {
      payload: { success: true, executionStatus: 'cancelled', reason: 'user_abort' },
    })
    return
  }

  abortRequested = false
  action('busy-change', { busy: true, command: command.type })
  try {
    if (command.type === 'send') {
      const prompt = String(command.prompt || '').trim()
      if (!prompt) return
      pendingPrompt.value = prompt
      if (!localSessionId.value) await createSession(prompt)
      if (abortRequested || disposed) {
        pendingPrompt.value = ''
        return
      }
      await nextTick()
      await waitUntilConnected()
      if (abortRequested || disposed) {
        pendingPrompt.value = ''
        await conversationRef.value?.cancelPlan?.()
        return
      }
      const attachments = await uploadAttachments(command)
      if (abortRequested || disposed) {
        pendingPrompt.value = ''
        await conversationRef.value?.cancelPlan?.()
        return
      }
      await conversationRef.value.sendChat(prompt, {
        requestId: command.id,
        model: effectiveModel.value,
        attachments,
        thinking: command.thinking,
        userOverride: 'plan',
        metadata: command.metadata,
      })
      if (abortRequested || disposed) {
        pendingPrompt.value = ''
        await conversationRef.value?.cancelPlan?.()
        return
      }
      pendingPrompt.value = ''
      action('message-sent', { prompt })
      return
    }
    const instance = conversationRef.value
    if (!instance) throw new Error('FastPlan 时间线节点尚未挂载')
    if (command.type === 'refresh') await instance.loadSession(localSessionId.value, { force: true, connect: true })
    else if (command.type === 'ensure-live') {
      const runtime = instance.runtime
      // KeepAlive 切回时内存状态仍是最新的。只在连接确实丢失时重连，
      // 不清空状态，也不重新下载完整 session / execution-state。
      if (!runtime?.loading?.value && !runtime?.connected?.value) runtime?.connect?.(localSessionId.value)
    }
    else if (command.type === 'pause') await instance.pauseExecution()
    else if (command.type === 'resume') await instance.resumeExecution()
    else if (command.type === 'confirm') await instance.confirmPlan?.()
    else if (command.type === 'retry-node') await instance.retryNode(command.nodeId)
    else if (command.type === 'skip-node') await instance.skipNode(command.nodeId)
  } catch (error) {
    if (abortRequested || disposed) return
    const normalized = normalizeError(error)
    pendingError.value = normalized.message
    action('error', { error: normalized })
  } finally {
    // stop 在进入 try 前已 return，不会走到这里。
    // send / confirm / resume 的 busy 由后续 state-change 维持，abort 时强制熄灭。
    if (abortRequested || (command.type !== 'send' && command.type !== 'confirm' && command.type !== 'resume')) {
      action('busy-change', { busy: false, command: command.type })
    }
  }
}

function handleStateChange(payload: unknown) {
  const state = conversationRef.value?.state
  const ownsLiveComposer = effectiveConnectionMode.value === 'live'
    && (!props.config.composerEntryId || props.config.composerEntryId === props.entry.id)
  const busy = ownsLiveComposer
    && !!(conversationRef.value?.runtime?.loading?.value || state?.isPlanning || state?.isPreviewing || state?.isPaused || (state?.plannedTree && !state?.treeCompleted))
  action('state-change', { payload, state, busy, treeCompleted: !!state?.treeCompleted })
  action('scroll-request', { reason: (payload as any)?.event?.type || 'state-change' })
}

function handleNavigationChange(items: ChatEmbeddedNavigationItem[]) {
  const result = Array.isArray(items) ? items.map(item => ({ ...item })) : []
  const displayPrompt = String(props.data.prompt || '').trim()
  if (effectiveConnectionMode.value === 'live' && displayPrompt && result.length) {
    result[result.length - 1].label = displayPrompt
  }
  action('navigation-change', { items: result })
}

watch(() => props.data.sessionId, value => {
  const id = String(value || '')
  if (id && id !== localSessionId.value) localSessionId.value = id
})
watch(() => props.data.command, command => runCommand(command), { immediate: true, deep: true })
watch(
  [localSessionId, effectiveConnectionMode, () => props.config.sessionManager],
  ([sessionId, mode, manager], _previous, onCleanup) => {
    if (!manager || !sessionId || mode !== 'live') return
    manager.retain?.(String(sessionId), runtimeOwnerId)
    onCleanup(() => manager.release?.(String(sessionId), runtimeOwnerId))
  },
  { immediate: true },
)
onBeforeUnmount(() => {
  disposed = true
})
</script>

<template>
  <section class="fastplan-timeline-renderer">
    <button v-if="localSessionId && !shouldMountConversation" type="button"
      class="fastplan-timeline-renderer__history-summary" @click="historyExpanded = true">
      <span class="fastplan-timeline-renderer__history-copy">
        <strong>{{ historySummary }}</strong>
        <small>历史规划尚未加载，点击查看完整任务树与执行记录</small>
      </span>
      <span class="fastplan-timeline-renderer__history-action">展开</span>
    </button>
    <FastPlanConversation v-else-if="localSessionId" :key="`${localSessionId}:${managedRuntime ? 'managed' : 'local'}`" ref="conversationRef"
      :session-id="localSessionId" :runtime="managedRuntime"
      :api-base="config.apiBase" :access="config.access" :ws-base="config.wsBase"
      :upload-base="config.uploadBase || config.apiBase" :admin-base="config.adminBase || config.apiBase"
      :download-base="config.downloadBase || config.apiBase" :execution-mode="effectiveExecutionMode"
      :confirmation-policy="effectiveConfirmationPolicy" :max-reconnect-attempts="config.maxReconnectAttempts"
      :base-reconnect-delay-ms="config.baseReconnectDelayMs"
      :retain-connection-on-session-switch="config.retainConnectionOnSessionSwitch" :model="effectiveModel"
      :theme="config.theme || 'light'" :theme-tokens="themeTokens" :connection-mode="effectiveConnectionMode"
      :density="config.density || 'auto'" :compact-breakpoint="config.compactBreakpoint || 700"
      :locale="config.locale" :labels="config.labels" :planning-labels="config.planningLabels"
      layout="embedded" appearance="seamless" :composer-mode="effectiveComposerMode"
      :composer-target="config.composerTarget" :show-task-list="effectiveShowTaskList" interaction-placement="inline"
      error-placement="inline" scroll-mode="host" :show-composer="false"
      :initial-feedback="initialFeedback"
      :before-send="config.beforeSend"
      :on-control="interactionDispatcher"
      @state-change="handleStateChange"
      @navigation-change="handleNavigationChange"
      @loading-change="action('loading-change', { loading: $event })"
      @execution-completed="action('execution-completed', { payload: $event })"
      @human-request="action('human-request', { payload: $event })"
      @node-select="action('node-select', { node: $event, state: $event.state || conversationRef?.state })"
      @message-action="(event) => action('message-action', { event })"
      @error="action('error', { error: normalizeError($event) })">
      <template v-if="avatarUrl" #avatar><img class="fastplan-timeline-renderer__avatar" :src="avatarUrl"
          alt="" /></template>
      <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
    </FastPlanConversation>
    <UntypedChatConversation v-if="displayedPendingPrompt" :messages="pendingMessages" :show-context="false"
      :show-composer="false" scroll-mode="host" appearance="seamless" :theme="config.theme || 'light'"
      :theme-tokens="themeTokens" :locale="config.locale" :labels="config.labels">
      <template v-if="avatarUrl" #avatar><img class="fastplan-timeline-renderer__avatar" :src="avatarUrl"
          alt="" /></template>
    </UntypedChatConversation>
    <p v-if="displayedPendingError" class="fastplan-timeline-renderer__error" role="alert">{{ displayedPendingError }}</p>
    <p v-else-if="data.uploadProgress" class="fastplan-timeline-renderer__upload-progress">附件上传中 {{ Math.round(data.uploadProgress.progress) }}%</p>
  </section>
</template>

<style scoped>
.fastplan-timeline-renderer {
  width: 100%;
  min-width: 0;
  background: transparent;
}

.fastplan-timeline-renderer__avatar {
  display: block;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  object-fit: cover;
}

.fastplan-timeline-renderer__error {
  margin: 8px 48px 0 0;
  color: var(--jcr-danger, #dc2626);
  font-size: 13px;
  text-align: right;
}

.fastplan-timeline-renderer__history-summary {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 16px;
  border: 1px solid var(--jcr-border, rgba(148, 163, 184, 0.28));
  border-radius: 10px;
  background: var(--jcr-surface-muted, rgba(148, 163, 184, 0.08));
  color: var(--jcr-text, inherit);
  text-align: left;
  cursor: pointer;
}

.fastplan-timeline-renderer__history-summary:hover {
  border-color: var(--jcr-primary, #409eff);
}

.fastplan-timeline-renderer__history-copy {
  display: grid;
  min-width: 0;
  gap: 4px;
}

.fastplan-timeline-renderer__history-copy strong {
  overflow: hidden;
  font-size: 14px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fastplan-timeline-renderer__history-copy small {
  color: var(--jcr-text-secondary, #64748b);
  font-size: 12px;
}

.fastplan-timeline-renderer__history-action {
  flex: none;
  color: var(--jcr-primary, #409eff);
  font-size: 13px;
}
</style>

<style>
/* The outer ChatWorkspace owns the empty state. Do not render a second empty
   placeholder inside an embedded FastPlan timeline entry. */
.fastplan-timeline-renderer .fastplan-conversation .jcr-state--empty {
  display: none;
}
</style>

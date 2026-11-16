<script setup lang="ts">
import { computed } from 'vue'
import ChatMessageItem from '../ChatMessageItem.vue'
import ChatMessageHeader from '../ChatMessageHeader.vue'
import ChatToolGroup from '../ChatToolGroup.vue'
import { groupMessagesIntoTurns, resolveMessageActions } from '../core'
import { defaultMessages } from '../messages'
import type {
  ActionResolver, AssistantActionsReady, ChatMessage, ChatRendererAdapters, ChatRendererMessages, ImagePart,
  LoadingState, MessageActionEvent, MessageActionsConfig, ToolRendererRegistry,
} from '../types'

const props = withDefaults(defineProps<{
  messages: ChatMessage[]
  loadingState?: LoadingState
  showActions?: boolean
  assistantActionsReady?: AssistantActionsReady
  actions?: MessageActionsConfig
  resolveActions?: ActionResolver
  adapters?: ChatRendererAdapters
  toolRenderers?: ToolRendererRegistry
  labels?: Partial<ChatRendererMessages>
  theme?: 'light' | 'dark'
  allowHtml?: boolean
  maxRenderChars?: number
  markdownAutoFoldThreshold?: number
  markdownShowCodeRowNumber?: boolean
  lastCompletedAssistantId?: string
}>(), {
  loadingState: () => ({}), showActions: true, assistantActionsReady: true,
  actions: () => ({}), toolRenderers: () => ({}), labels: () => ({}),
  theme: 'light', allowHtml: false, maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true, lastCompletedAssistantId: undefined,
})

defineEmits<{
  (event: 'message-action', value: MessageActionEvent): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: ImagePart[], activeIndex: number): void
  (event: 'show-full-content', message: ChatMessage): void
}>()

const turns = computed(() => groupMessagesIntoTurns(props.messages))
const messagesText = computed(() => ({ ...defaultMessages, ...props.labels }))
type ListItem = { kind: 'message'; message: ChatMessage } | { kind: 'tool-group'; id: string; messages: ChatMessage[] }

function listItems(messages: ChatMessage[]): ListItem[] {
  const result: ListItem[] = []
  for (let index = 0; index < messages.length;) {
    const message = messages[index]
    if (message.role !== 'tool' || message.status === 'pending' || message.status === 'streaming') {
      result.push({ kind: 'message', message })
      index += 1
      continue
    }
    const run: ChatMessage[] = []
    while (index < messages.length && messages[index].role === 'tool'
      && messages[index].status !== 'pending' && messages[index].status !== 'streaming') {
      run.push(messages[index++])
    }
    if (run.length > 1) result.push({ kind: 'tool-group', id: `tools-${run[0].id}`, messages: run })
    else result.push({ kind: 'message', message: run[0] })
  }
  return result
}

const localLastCompletedAssistantIndex = computed(() => {
  for (let index = props.messages.length - 1; index >= 0; index -= 1) {
    const message = props.messages[index]
    if (message.role === 'assistant' && message.status !== 'streaming' && message.status !== 'pending') return index
  }
  return -1
})

function actionsFor(message: ChatMessage, index: number) {
  const turn = turns.value.find(item => item.messages.includes(message))
  const assistantActionOwner = turn?.messages.slice().reverse().find(entry =>
    entry.role === 'assistant'
    && entry.status !== 'pending'
    && entry.status !== 'streaming'
    && entry.parts.some(part => part.type === 'text' && !!part.text.trim()),
  )
  if (message.role === 'assistant' && assistantActionOwner !== message) return []
  const isLastCompleted = props.lastCompletedAssistantId
    ? message.id === props.lastCompletedAssistantId
    : index === localLastCompletedAssistantIndex.value
  const context = {
    index,
    turnId: turn?.id,
    isLastMessage: index === props.messages.length - 1,
    isLastCompletedAssistantTurn: isLastCompleted,
    isGenerating: !!props.loadingState.generating || message.status === 'streaming' || message.status === 'pending',
    hasStableId: !!message.id && !String(message.id).startsWith('temporary-'),
  }
  const assistantReady = typeof props.assistantActionsReady === 'function'
    ? props.assistantActionsReady(message, context)
    : props.assistantActionsReady
  if (message.role === 'assistant' && !assistantReady) return []
  return resolveMessageActions(message, context, props.showActions, props.actions, props.resolveActions)
}

function isFirstAssistantInTurn(message: ChatMessage, turn: { messages: ChatMessage[] }) {
  return message.role === 'assistant' && turn.messages.find(entry => entry.role === 'assistant') === message
}
function firstModelOutput(turn: { messages: ChatMessage[] }) {
  return turn.messages.find(message => message.role === 'assistant' || message.role === 'tool')
}
function hasLeadingToolOutput(turn: { messages: ChatMessage[] }) {
  return firstModelOutput(turn)?.role === 'tool'
}
function modelHeaderMessage(turn: { messages: ChatMessage[] }): ChatMessage {
  return turn.messages.find(message => message.role === 'assistant') || firstModelOutput(turn) || turn.messages[0]
}
function itemContainsMessage(item: ListItem, message: ChatMessage) {
  return item.kind === 'message' ? item.message === message : item.messages.includes(message)
}
function showLeadingModelHeader(item: ListItem, turn: { messages: ChatMessage[] }) {
  const firstOutput = firstModelOutput(turn)
  return !!firstOutput && firstOutput.role === 'tool' && itemContainsMessage(item, firstOutput)
}
function showInlineModelHeader(message: ChatMessage, turn: { messages: ChatMessage[] }) {
  return isFirstAssistantInTurn(message, turn) && !hasLeadingToolOutput(turn)
}
</script>

<template>
  <div v-for="turn in turns" :key="turn.id" class="jcr-turn" :data-turn-id="turn.id">
    <template v-for="item in listItems(turn.messages)" :key="item.kind === 'message' ? item.message.id : item.id">
      <template v-if="showLeadingModelHeader(item, turn)">
        <slot v-if="$slots.header" name="header" :message="modelHeaderMessage(turn)" />
        <ChatMessageHeader v-else :message="modelHeaderMessage(turn)" :adapters="adapters" fallback-label="assistant" />
      </template>
      <ChatToolGroup
        v-if="item.kind === 'tool-group'" :messages="item.messages" :labels="messagesText"
        :adapters="adapters" :tool-renderers="toolRenderers" :allow-html="allowHtml"
        :max-render-chars="maxRenderChars" :theme="theme"
        :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
        :markdown-show-code-row-number="markdownShowCodeRowNumber"
        @message-action="$emit('message-action', $event)" @link-click="(href, event) => $emit('link-click', href, event)"
        @image-preview="(images, index) => $emit('image-preview', images, index)" @show-full-content="$emit('show-full-content', $event)"
      >
        <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
        <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
        <template v-if="$slots.content" #content="scope"><slot name="content" v-bind="scope" :turn="turn" :message-index="messages.indexOf(scope.message)" /></template>
        <template v-if="$slots.text" #text="scope"><slot name="text" v-bind="scope" :turn="turn" :message-index="messages.indexOf(scope.message)" /></template>
        <template v-if="$slots.reasoning" #reasoning="scope"><slot name="reasoning" v-bind="scope" :turn="turn" :message-index="messages.indexOf(scope.message)" /></template>
        <template v-if="$slots.attachment" #attachment="scope"><slot name="attachment" v-bind="scope" /></template>
        <template v-if="$slots.tool" #tool="scope"><slot name="tool" v-bind="scope" /></template>
        <template v-if="$slots.custom" #custom="scope"><slot name="custom" v-bind="scope" /></template>
      </ChatToolGroup>
      <ChatMessageItem
        v-else :message="item.message" :show-model-header="showInlineModelHeader(item.message, turn)"
        :show-avatar="item.message.role === 'user'" :actions="actionsFor(item.message, messages.indexOf(item.message))"
        :messages="messagesText" :adapters="adapters" :tool-renderers="toolRenderers" :allow-html="allowHtml"
        :max-render-chars="maxRenderChars" :theme="theme"
        :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
        :markdown-show-code-row-number="markdownShowCodeRowNumber"
        @message-action="$emit('message-action', $event)" @link-click="(href, event) => $emit('link-click', href, event)"
        @image-preview="(images, index) => $emit('image-preview', images, index)" @show-full-content="$emit('show-full-content', $event)"
      >
        <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
        <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
        <template v-if="$slots.content" #content="scope"><slot name="content" v-bind="scope" :turn="turn" :message-index="messages.indexOf(item.message)" /></template>
        <template v-if="$slots.text" #text="scope"><slot name="text" v-bind="scope" :turn="turn" :message-index="messages.indexOf(item.message)" /></template>
        <template v-if="$slots.reasoning" #reasoning="scope"><slot name="reasoning" v-bind="scope" :turn="turn" :message-index="messages.indexOf(item.message)" /></template>
        <template v-if="$slots.attachment" #attachment="scope"><slot name="attachment" v-bind="scope" /></template>
        <template v-if="$slots.tool" #tool="scope"><slot name="tool" v-bind="scope" /></template>
        <template v-if="$slots.custom" #custom="scope"><slot name="custom" v-bind="scope" /></template>
        <template v-if="$slots['before-content']" #before-content="scope"><slot name="before-content" v-bind="scope" :turn="turn" :message-index="messages.indexOf(item.message)" :is-first-assistant-in-turn="isFirstAssistantInTurn(item.message, turn)" /></template>
        <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
        <template v-if="$slots['action-bar']" #action-bar="scope"><slot name="action-bar" v-bind="scope" /></template>
        <template v-if="$slots.action" #action="scope"><slot name="action" v-bind="scope" /></template>
        <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
        <template v-if="$slots['action-menu']" #action-menu="scope"><slot name="action-menu" v-bind="scope" /></template>
      </ChatMessageItem>
      <slot v-if="item.kind === 'message'" name="message-after" :message="item.message" :turn="turn" :message-index="messages.indexOf(item.message)" />
    </template>
  </div>
</template>

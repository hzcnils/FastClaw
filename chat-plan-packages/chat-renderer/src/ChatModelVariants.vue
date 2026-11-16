<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import ChatMessageList from './components/ChatMessageList.vue'
import ChatPartPluginRenderer from './components/ChatPartPluginRenderer.vue'
import { defaultMessages } from './messages'
import type {
  ActionResolver, AssistantActionsReady, ChatComparisonModeChangeEvent, ChatRendererAdapters, ChatRendererMessages,
  ChatTurnComparisonMode, ChatTurnTimelineEntry, ChatVariantChangeEvent, LoadingState,
  MessageActionsConfig, ToolRendererRegistry,
  ChatWorkspacePlugins,
} from './types'

const props = withDefaults(defineProps<{
  turn: ChatTurnTimelineEntry
  activeId?: string
  mode?: ChatTurnComparisonMode
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
  plugins?: ChatWorkspacePlugins
}>(), {
  loadingState: () => ({}), showActions: true, assistantActionsReady: true,
  actions: () => ({}), toolRenderers: () => ({}), labels: () => ({}), plugins: () => ({}),
  theme: 'light', allowHtml: false, maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true,
})

const emit = defineEmits<{
  (event: 'variant-change', value: ChatVariantChangeEvent): void
  (event: 'comparison-mode-change', value: ChatComparisonModeChangeEvent): void
  (event: 'message-action', value: unknown): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: unknown[], activeIndex: number): void
  (event: 'show-full-content', message: unknown): void
}>()

const text = computed(() => ({ ...defaultMessages, ...props.labels }))
const initialActive = () => props.activeId || props.turn.activeId || props.turn.alternatives[0]?.id || ''
const localActiveId = ref(initialActive())
const localMode = ref<ChatTurnComparisonMode>(props.mode || props.turn.comparisonMode || (props.turn.alternatives.length > 1 ? 'tabs' : 'single'))

watch(() => [props.activeId, props.turn.activeId, props.turn.alternatives] as const, () => {
  const candidate = initialActive()
  localActiveId.value = props.turn.alternatives.some(item => item.id === candidate) ? candidate : (props.turn.alternatives[0]?.id || '')
}, { deep: true })
watch(() => [props.mode, props.turn.comparisonMode] as const, () => {
  localMode.value = props.mode || props.turn.comparisonMode || (props.turn.alternatives.length > 1 ? 'tabs' : 'single')
})

const activeAlternative = computed(() => props.turn.alternatives.find(item => item.id === localActiveId.value) || props.turn.alternatives[0])

function selectVariant(activeId: string) {
  if (activeId === localActiveId.value) return
  const previousActiveId = localActiveId.value || undefined
  localActiveId.value = activeId
  emit('variant-change', { turnId: props.turn.id, activeId, previousActiveId })
}

function selectMode(mode: ChatTurnComparisonMode) {
  if (mode === localMode.value) return
  localMode.value = mode
  emit('comparison-mode-change', { turnId: props.turn.id, mode })
}
</script>

<template>
  <section class="jcr-variants" :data-entry-id="turn.id" :data-turn-id="turn.id" :data-mode="localMode">
    <slot v-if="!turn.alternatives.length" name="variant-empty" :turn="turn" />
    <div v-else-if="localMode === 'split' && turn.alternatives.length > 1" class="jcr-variants__split">
      <ChatMessageList class="jcr-variants__user" :messages="[turn.user]" :loading-state="loadingState"
        :show-actions="showActions" :assistant-actions-ready="assistantActionsReady" :actions="actions"
        :resolve-actions="resolveActions" :adapters="adapters" :tool-renderers="toolRenderers" :labels="labels"
        :theme="theme" :allow-html="allowHtml" :max-render-chars="maxRenderChars"
        :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
        :markdown-show-code-row-number="markdownShowCodeRowNumber"
        :last-completed-assistant-id="lastCompletedAssistantId" @message-action="emit('message-action', $event)"
        @link-click="(href, event) => emit('link-click', href, event)"
        @image-preview="(images, index) => emit('image-preview', images, index)"
        @show-full-content="emit('show-full-content', $event)">
        <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
        <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
        <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
        <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
        <template #custom="scope"><slot name="custom" v-bind="scope"><ChatPartPluginRenderer v-bind="scope" :plugins="plugins" :labels="labels" /></slot></template>
      </ChatMessageList>
      <header class="jcr-variants__toolbar">
        <div class="jcr-variants__tabs" role="tablist">
          <button v-for="alternative in turn.alternatives" :key="alternative.id" type="button" role="tab"
            :aria-selected="alternative.id === localActiveId" :class="{ 'is-active': alternative.id === localActiveId }"
            @click="selectVariant(alternative.id)">
            <slot name="variant-label" :message="alternative" :active="alternative.id === localActiveId">
              {{ alternative.model?.name || alternative.model?.id || alternative.id }}
            </slot>
          </button>
        </div>
        <div class="jcr-variants__modes">
          <button type="button" :class="{ 'is-active': localMode !== 'split' }" :title="text.tabs"
            :aria-label="text.tabs" @click="selectMode('tabs')">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v5H4zM4 14h16v5H4z" /></svg>
          </button>
          <button type="button" :class="{ 'is-active': localMode === 'split' }" :title="text.split"
            :aria-label="text.split" @click="selectMode('split')">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h7v16H4zM13 4h7v16h-7z" /></svg>
          </button>
        </div>
        <slot name="variant-toolbar" :turn="turn" :mode="localMode" />
      </header>
      <div class="jcr-variants__grid">
        <article v-for="alternative in turn.alternatives" :key="alternative.id" class="jcr-variants__column" :data-variant-id="alternative.id">
          <slot name="variant" :turn="turn" :message="alternative" :active="alternative.id === localActiveId">
            <ChatMessageList :messages="[alternative]" :loading-state="loadingState" :show-actions="showActions"
              :assistant-actions-ready="assistantActionsReady" :actions="actions" :resolve-actions="resolveActions"
              :adapters="adapters" :tool-renderers="toolRenderers" :labels="labels" :theme="theme"
              :allow-html="allowHtml" :max-render-chars="maxRenderChars" :last-completed-assistant-id="lastCompletedAssistantId"
              :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
              :markdown-show-code-row-number="markdownShowCodeRowNumber"
              @message-action="emit('message-action', $event)" @link-click="(href, event) => emit('link-click', href, event)"
              @image-preview="(images, index) => emit('image-preview', images, index)"
              @show-full-content="emit('show-full-content', $event)">
              <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
              <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
              <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
              <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
              <template #custom="scope"><slot name="custom" v-bind="scope"><ChatPartPluginRenderer v-bind="scope" :plugins="plugins" :labels="labels" /></slot></template>
            </ChatMessageList>
          </slot>
        </article>
      </div>
    </div>
    <template v-else-if="$slots.variant">
      <header v-if="turn.alternatives.length > 1" class="jcr-variants__toolbar">
        <div class="jcr-variants__tabs" role="tablist">
          <button v-for="alternative in turn.alternatives" :key="alternative.id" type="button" role="tab"
            :aria-selected="alternative.id === localActiveId" :class="{ 'is-active': alternative.id === localActiveId }"
            @click="selectVariant(alternative.id)">
            <slot name="variant-label" :message="alternative" :active="alternative.id === localActiveId">
              {{ alternative.model?.name || alternative.model?.id || alternative.id }}
            </slot>
          </button>
        </div>
        <div class="jcr-variants__modes">
          <button type="button" :class="{ 'is-active': localMode !== 'split' }" :title="text.tabs"
            :aria-label="text.tabs" @click="selectMode('tabs')">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v5H4zM4 14h16v5H4z" /></svg>
          </button>
          <button type="button" :class="{ 'is-active': localMode === 'split' }" :title="text.split"
            :aria-label="text.split" @click="selectMode('split')">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h7v16H4zM13 4h7v16h-7z" /></svg>
          </button>
        </div>
        <slot name="variant-toolbar" :turn="turn" :mode="localMode" />
      </header>
      <slot name="variant" :turn="turn" :message="activeAlternative" :active="true" />
    </template>
    <template v-else>
      <ChatMessageList :messages="[turn.user]" :loading-state="loadingState"
        :show-actions="showActions" :assistant-actions-ready="assistantActionsReady" :actions="actions"
        :resolve-actions="resolveActions" :adapters="adapters" :tool-renderers="toolRenderers" :labels="labels"
        :theme="theme" :allow-html="allowHtml" :max-render-chars="maxRenderChars"
        :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
        :markdown-show-code-row-number="markdownShowCodeRowNumber"
        :last-completed-assistant-id="lastCompletedAssistantId" @message-action="emit('message-action', $event)"
        @link-click="(href, event) => emit('link-click', href, event)"
        @image-preview="(images, index) => emit('image-preview', images, index)"
        @show-full-content="emit('show-full-content', $event)">
        <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
        <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
        <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
        <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
        <template #custom="scope"><slot name="custom" v-bind="scope"><ChatPartPluginRenderer v-bind="scope" :plugins="plugins" :labels="labels" /></slot></template>
      </ChatMessageList>
      <header v-if="turn.alternatives.length > 1" class="jcr-variants__toolbar">
        <div class="jcr-variants__tabs" role="tablist">
          <button v-for="alternative in turn.alternatives" :key="alternative.id" type="button" role="tab"
            :aria-selected="alternative.id === localActiveId" :class="{ 'is-active': alternative.id === localActiveId }"
            @click="selectVariant(alternative.id)">
            <slot name="variant-label" :message="alternative" :active="alternative.id === localActiveId">
              {{ alternative.model?.name || alternative.model?.id || alternative.id }}
            </slot>
          </button>
        </div>
        <div class="jcr-variants__modes">
          <button type="button" :class="{ 'is-active': localMode !== 'split' }" :title="text.tabs"
            :aria-label="text.tabs" @click="selectMode('tabs')">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v5H4zM4 14h16v5H4z" /></svg>
          </button>
          <button type="button" :class="{ 'is-active': localMode === 'split' }" :title="text.split"
            :aria-label="text.split" @click="selectMode('split')">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h7v16H4zM13 4h7v16h-7z" /></svg>
          </button>
        </div>
        <slot name="variant-toolbar" :turn="turn" :mode="localMode" />
      </header>
      <ChatMessageList v-if="activeAlternative" :messages="[activeAlternative]" :loading-state="loadingState"
        :show-actions="showActions" :assistant-actions-ready="assistantActionsReady" :actions="actions"
        :resolve-actions="resolveActions" :adapters="adapters" :tool-renderers="toolRenderers" :labels="labels"
        :theme="theme" :allow-html="allowHtml" :max-render-chars="maxRenderChars"
        :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
        :markdown-show-code-row-number="markdownShowCodeRowNumber"
        :last-completed-assistant-id="lastCompletedAssistantId" @message-action="emit('message-action', $event)"
        @link-click="(href, event) => emit('link-click', href, event)"
        @image-preview="(images, index) => emit('image-preview', images, index)"
        @show-full-content="emit('show-full-content', $event)">
        <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
        <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
        <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
        <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
        <template #custom="scope"><slot name="custom" v-bind="scope"><ChatPartPluginRenderer v-bind="scope" :plugins="plugins" :labels="labels" /></slot></template>
      </ChatMessageList>
    </template>
  </section>
</template>

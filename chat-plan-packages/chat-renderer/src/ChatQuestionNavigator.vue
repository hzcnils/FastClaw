<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { resolveChatRendererMessages } from './messages'
import type { ChatQuestionNavigateEvent, ChatQuestionNavigationItem, ChatRendererMessages } from './types'

let navigatorId = 0

const props = withDefaults(defineProps<{
  items?: ChatQuestionNavigationItem[]
  activeId?: string
  collapsed?: boolean
  ariaLabel?: string
  locale?: string
  labels?: Partial<ChatRendererMessages>
}>(), {
  items: () => [],
  collapsed: false,
  ariaLabel: undefined,
  locale: 'zh-CN',
  labels: () => ({}),
})

const emit = defineEmits<{
  (event: 'navigate', value: ChatQuestionNavigateEvent): void
}>()

const root = ref<HTMLElement | null>(null)
const panelOpen = ref(false)
const panelId = `jcr-question-directory-${++navigatorId}`
const text = computed(() => resolveChatRendererMessages(props.locale, props.labels))
const effectiveAriaLabel = computed(() => props.ariaLabel || text.value.questionDirectory)

function closePanel() {
  panelOpen.value = false
}

function togglePanel() {
  panelOpen.value = !panelOpen.value
}

function navigate(item: ChatQuestionNavigationItem, nativeEvent: MouseEvent | KeyboardEvent) {
  if (item.disabled) return
  emit('navigate', { item, nativeEvent })
}

function navigateFromPanel(item: ChatQuestionNavigationItem, nativeEvent: MouseEvent | KeyboardEvent) {
  navigate(item, nativeEvent)
  if (!item.disabled) closePanel()
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (panelOpen.value && !root.value?.contains(event.target as Node)) closePanel()
}

function handleDocumentKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && panelOpen.value) closePanel()
}

watch(() => props.items.length, length => {
  if (length <= 1) closePanel()
})
watch(panelOpen, open => {
  if (open) {
    document.addEventListener('pointerdown', handleDocumentPointerDown)
    document.addEventListener('keydown', handleDocumentKeydown)
  } else {
    document.removeEventListener('pointerdown', handleDocumentPointerDown)
    document.removeEventListener('keydown', handleDocumentKeydown)
  }
})
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', handleDocumentPointerDown)
  document.removeEventListener('keydown', handleDocumentKeydown)
})
</script>

<template>
  <nav v-if="items.length > 1" ref="root" class="jcr-question-nav" :class="{ 'is-collapsed': collapsed }" :aria-label="effectiveAriaLabel">
    <button class="jcr-question-nav__directory" type="button" :aria-controls="panelId" :aria-expanded="panelOpen"
      :aria-label="panelOpen ? text.closeQuestionDirectory : text.openQuestionDirectory"
      :title="panelOpen ? text.closeQuestionDirectory : text.openQuestionDirectory" @click="togglePanel">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 5.5h2M9 5.5h11M4 12h2M9 12h11M4 18.5h2M9 18.5h11" />
      </svg>
    </button>

    <div class="jcr-question-nav__rail">
      <ol>
        <li v-for="item in items" :key="item.id">
          <button class="jcr-question-nav__rail-item" type="button" :disabled="item.disabled" :class="{ 'is-active': item.id === activeId }"
            :aria-current="item.id === activeId ? 'true' : undefined" :title="item.label"
            @click="navigate(item, $event)">
            <span class="jcr-question-nav__index">{{ item.index + 1 }}</span>
            <span v-if="!collapsed" class="jcr-question-nav__label">{{ item.label }}</span>
          </button>
        </li>
      </ol>
    </div>

    <section v-if="panelOpen" :id="panelId" class="jcr-question-nav__panel" role="dialog" :aria-label="text.questionDirectory">
      <ol>
        <li v-for="item in items" :key="`directory-${item.id}`">
          <button class="jcr-question-nav__panel-item" type="button" :disabled="item.disabled"
            :class="{ 'is-active': item.id === activeId }" :aria-current="item.id === activeId ? 'true' : undefined"
            :title="item.label" @click="navigateFromPanel(item, $event)">
            <span class="jcr-question-nav__panel-index">{{ item.index + 1 }}</span>
            <span class="jcr-question-nav__panel-label">{{ item.label }}</span>
          </button>
        </li>
      </ol>
    </section>
  </nav>
</template>

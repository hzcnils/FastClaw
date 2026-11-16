<script setup lang="ts">
import { computed } from 'vue'
import { visibleContextItems as resolveVisibleContextItems } from './core'
import type { ChatContextActionEvent, ChatContextItem, ChatContextPlacement } from './types'

const props = withDefaults(defineProps<{
  items?: ChatContextItem[]
  placement?: ChatContextPlacement
  ariaLabel?: string
  theme?: 'light' | 'dark'
}>(), { items: () => [], placement: 'top', ariaLabel: 'Conversation context', theme: 'light' })

const emit = defineEmits<{
  (event: 'context-action', value: ChatContextActionEvent): void
}>()

const visibleItems = computed(() => resolveVisibleContextItems(props.items))

function select(item: ChatContextItem, nativeEvent: MouseEvent | KeyboardEvent) {
  if (item.disabled) return
  emit('context-action', { action: 'select', item, nativeEvent })
}

function onKeydown(item: ChatContextItem, event: KeyboardEvent) {
  if (event.key !== 'Enter' && event.key !== ' ') return
  event.preventDefault()
  select(item, event)
}
</script>

<template>
  <nav class="jcr-context-bar" :data-placement="placement" :data-theme="theme" :aria-label="ariaLabel">
    <div class="jcr-context-bar__start">
      <slot name="start" />
      <template v-for="item in visibleItems.filter(entry => ['document', 'knowledge'].includes(entry.kind))" :key="item.id">
        <slot name="item" :item="item" :select="select">
          <button type="button" class="jcr-context-item" :class="{ 'is-active': item.active }" :disabled="item.disabled"
            :aria-pressed="item.active || undefined" @click="select(item, $event)" @keydown="onKeydown(item, $event)">
            <span v-if="item.icon" class="jcr-context-item__icon" aria-hidden="true">{{ item.icon }}</span>
            <span>{{ item.label }}</span><strong v-if="item.value">{{ item.value }}</strong>
            <small v-if="item.badge !== undefined">{{ item.badge }}</small>
          </button>
        </slot>
      </template>
    </div>
    <div class="jcr-context-bar__end">
      <template v-for="item in visibleItems.filter(entry => !['document', 'knowledge'].includes(entry.kind))" :key="item.id">
        <slot name="item" :item="item" :select="select">
          <button type="button" class="jcr-context-item" :class="{ 'is-active': item.active }" :disabled="item.disabled"
            :aria-pressed="item.active || undefined" @click="select(item, $event)" @keydown="onKeydown(item, $event)">
            <span v-if="item.icon" class="jcr-context-item__icon" aria-hidden="true">{{ item.icon }}</span>
            <span>{{ item.label }}</span><strong v-if="item.value">{{ item.value }}</strong>
            <small v-if="item.badge !== undefined">{{ item.badge }}</small>
          </button>
        </slot>
      </template>
      <slot name="end" />
    </div>
  </nav>
</template>

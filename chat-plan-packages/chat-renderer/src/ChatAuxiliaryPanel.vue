<script setup lang="ts">
import { computed } from 'vue'
import RendererBoundary from './RendererBoundary.vue'
import { defaultMessages } from './messages'
import type { ChatAuxiliaryItem, ChatAuxiliaryPlacement, ChatRendererMessages, ChatWorkspacePlugins } from './types'

const props = withDefaults(defineProps<{
  items?: ChatAuxiliaryItem[]
  plugins?: ChatWorkspacePlugins
  activeId?: string
  placement?: ChatAuxiliaryPlacement
  labels?: Partial<ChatRendererMessages>
}>(), {
  items: () => [],
  plugins: () => ({}),
  placement: 'side',
  labels: () => ({}),
})

const emit = defineEmits<{
  (event: 'update:activeId', value: string): void
  (event: 'close', item: ChatAuxiliaryItem): void
  (event: 'action', payload: unknown, item: ChatAuxiliaryItem): void
}>()

const text = computed(() => ({ ...defaultMessages, ...props.labels }))
const visibleItems = computed(() => props.items.filter(item => item.visible !== false))
const activeItem = computed(() => visibleItems.value.find(item => item.id === props.activeId) || visibleItems.value[0])
const activeRenderer = computed(() => activeItem.value ? props.plugins.auxiliaryPanels?.[activeItem.value.renderer] : undefined)
</script>

<template>
  <aside v-if="activeItem" class="jcr-auxiliary" :data-placement="placement">
    <header class="jcr-auxiliary__header">
      <div v-if="visibleItems.length > 1" class="jcr-auxiliary__tabs" role="tablist">
        <button v-for="item in visibleItems" :key="item.id" type="button" role="tab"
          :aria-selected="item.id === activeItem.id" :class="{ 'is-active': item.id === activeItem.id }"
          @click="emit('update:activeId', item.id)">{{ item.title || item.renderer }}</button>
      </div>
      <strong v-else>{{ activeItem.title || activeItem.renderer }}</strong>
      <button v-if="activeItem.closable !== false" type="button" class="jcr-auxiliary__close"
        :aria-label="text.close" @click="emit('close', activeItem)">×</button>
    </header>
    <div class="jcr-auxiliary__body">
      <slot name="panel" :item="activeItem" :renderer="activeRenderer">
        <RendererBoundary v-if="activeRenderer" :key="activeItem.id">
          <component :is="activeRenderer" :item="activeItem" :data="activeItem.data"
            @action="emit('action', $event, activeItem)" />
          <template #fallback="scope"><slot name="error" v-bind="scope" :item="activeItem" /></template>
        </RendererBoundary>
        <slot v-else name="missing-renderer" :item="activeItem">
          <p class="jcr-fallback">{{ text.missingRenderer }}: {{ activeItem.renderer }}</p>
        </slot>
      </slot>
    </div>
  </aside>
</template>

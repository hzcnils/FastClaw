<script setup lang="ts">
import { computed } from 'vue'
import type { ChatActionIconName } from './types'
type SvgNode = readonly [tag: 'path' | 'rect' | 'circle' | 'line', attrs: Record<string, string | number>]

const props = defineProps<{
  name: ChatActionIconName
  copied?: boolean
  active?: boolean
  customIcon?: string
}>()

// Mirrors the Lucide icons used by the host MessageActionBar without adding a
// Lucide runtime dependency or coupling this package to the host's version.
const icons: Record<ChatActionIconName, SvgNode[]> = {
  copy: [
    ['rect', { width: 14, height: 14, x: 8, y: 8, rx: 2, ry: 2 }],
    ['path', { d: 'M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2' }],
  ],
  copied: [['path', { d: 'M16.707 5.293a1 1 0 0 1 0 1.414l-8 8a1 1 0 0 1-1.414 0l-4-4a1 1 0 0 1 1.414-1.414L8 12.586l7.293-7.293a1 1 0 0 1 1.414 0Z', fill: 'currentColor', stroke: 'none', 'fill-rule': 'evenodd', 'clip-rule': 'evenodd' }]],
  edit: [
    ['path', { d: 'M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497Z' }],
    ['path', { d: 'm15 5 4 4' }],
  ],
  resend: [
    ['path', { d: 'M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8' }],
    ['path', { d: 'M21 3v5h-5' }],
    ['path', { d: 'M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16' }],
    ['path', { d: 'M8 16H3v5' }],
  ],
  regenerate: [
    ['path', { d: 'M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8' }],
    ['path', { d: 'M21 3v5h-5' }],
    ['path', { d: 'M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16' }],
    ['path', { d: 'M8 16H3v5' }],
  ],
  translate: [
    ['path', { d: 'm5 8 6 6' }], ['path', { d: 'm4 14 6-6 2-3' }],
    ['path', { d: 'M2 5h12' }], ['path', { d: 'M7 2h1' }],
    ['path', { d: 'm22 22-5-10-5 10' }], ['path', { d: 'M14 18h6' }],
  ],
  favorite: [['path', { d: 'M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16Z' }]],
  'edit-as-doc': [
    ['path', { d: 'M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z' }],
    ['path', { d: 'M14 2v4a2 2 0 0 0 2 2h4' }], ['path', { d: 'M10 9H8' }],
    ['path', { d: 'M16 13H8' }], ['path', { d: 'M16 17H8' }],
  ],
  'switch-model': [
    ['path', { d: 'm17 2 4 4-4 4' }], ['path', { d: 'M3 11v-1a4 4 0 0 1 4-4h14' }],
    ['path', { d: 'm7 22-4-4 4-4' }], ['path', { d: 'M21 13v1a4 4 0 0 1-4 4H3' }],
  ],
  quote: [['path', { d: 'M20 18v-2a4 4 0 0 0-4-4H4' }], ['path', { d: 'm9 17-5-5 5-5' }]],
  tts: [
    ['path', { d: 'M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298Z' }],
    ['path', { d: 'M16 9a5 5 0 0 1 0 6' }], ['path', { d: 'M19.364 18.364a9 9 0 0 0 0-12.728' }],
  ],
  'trace-source': [['path', { d: 'm21 21-4.34-4.34' }], ['circle', { cx: 11, cy: 11, r: 8 }]],
  export: [
    ['path', { d: 'M12 15V3' }], ['path', { d: 'M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4' }],
    ['path', { d: 'm7 10 5 5 5-5' }],
  ],
  feedback: [
    ['path', { d: 'M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4Z' }],
    ['line', { x1: 8, x2: 16, y1: 9, y2: 9 }],
    ['line', { x1: 8, x2: 13, y1: 13, y2: 13 }],
  ],
  like: [
    ['path', { d: 'M7 10v12' }],
    ['path', { d: 'M15 5.88 14 10h5.83a2 2 0 0 1 1.92 2.56l-2.33 8A2 2 0 0 1 17.5 22H4a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h2.76a2 2 0 0 0 1.79-1.11L12 2a3.13 3.13 0 0 1 3 3.88Z' }],
  ],
  dislike: [
    ['path', { d: 'M17 14V2' }],
    ['path', { d: 'M9 18.12 10 14H4.17a2 2 0 0 1-1.92-2.56l2.33-8A2 2 0 0 1 6.5 2H20a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-2.76a2 2 0 0 0-1.79 1.11L12 22a3.13 3.13 0 0 1-3-3.88Z' }],
  ],
  branch: [
    ['line', { x1: 6, x2: 6, y1: 3, y2: 15 }], ['circle', { cx: 18, cy: 6, r: 3 }],
    ['circle', { cx: 6, cy: 18, r: 3 }], ['path', { d: 'M18 9a9 9 0 0 1-9 9' }],
  ],
  delete: [
    ['path', { d: 'M3 6h18' }], ['path', { d: 'M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6' }],
    ['path', { d: 'M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2' }],
    ['line', { x1: 10, x2: 10, y1: 11, y2: 17 }], ['line', { x1: 14, x2: 14, y1: 11, y2: 17 }],
  ],
  custom: [['circle', { cx: 5, cy: 12, r: 1 }], ['circle', { cx: 12, cy: 12, r: 1 }], ['circle', { cx: 19, cy: 12, r: 1 }]],
  'chevron-down': [['path', { d: 'm6 9 6 6 6-6' }]],
  loader: [['path', { d: 'M21 12a9 9 0 1 1-6.219-8.56' }]],
}

const iconAliases: Record<string, ChatActionIconName> = {
  trace: 'trace-source',
  'edit-doc': 'edit-as-doc',
  document: 'edit-as-doc',
}
const customBuiltInIcon = computed<ChatActionIconName | null>(() => {
  const value = String(props.customIcon || '').trim()
  if (!value) return null
  if (value in icons) return value as ChatActionIconName
  return iconAliases[value] || null
})
const textCustomIcon = computed(() => props.customIcon && !customBuiltInIcon.value ? props.customIcon : '')
const iconName = computed<ChatActionIconName>(() => props.copied ? 'copied' : customBuiltInIcon.value || props.name)
const nodes = computed(() => icons[iconName.value])
const viewBox = computed(() => iconName.value === 'copied' ? '0 0 20 20' : '0 0 24 24')
</script>

<template>
  <span v-if="textCustomIcon" class="jcr-action__custom-icon" aria-hidden="true">{{ textCustomIcon }}</span>
  <svg v-else class="jcr-action__svg" :class="{ 'jcr-action__svg--spin': iconName === 'loader' }"
    :data-action-icon="iconName" :data-active="active || undefined" :viewBox="viewBox" fill="none" stroke="currentColor"
    stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <component :is="node[0]" v-for="(node, index) in nodes" :key="index" v-bind="node[1]" />
  </svg>
</template>

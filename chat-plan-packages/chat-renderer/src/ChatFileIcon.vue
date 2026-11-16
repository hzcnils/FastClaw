<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  name?: string
  mimeType?: string
  generic?: boolean
}>(), {
  name: '',
  mimeType: '',
  generic: false,
})

const extension = computed(() => {
  const value = props.name.split('.').pop()?.toLowerCase() || ''
  if (value === 'pdf' || props.mimeType.includes('pdf')) return 'PDF'
  if (['doc', 'docx'].includes(value) || props.mimeType.includes('word')) return 'DOC'
  if (['xls', 'xlsx', 'csv'].includes(value) || props.mimeType.includes('sheet')) return value === 'csv' ? 'CSV' : 'XLS'
  if (['ppt', 'pptx'].includes(value) || props.mimeType.includes('presentation')) return 'PPT'
  if (value === 'md' || props.mimeType.includes('markdown')) return 'MD'
  if (['txt', 'text'].includes(value) || props.mimeType.startsWith('text/')) return 'TXT'
  if (['zip', 'rar', '7z', 'tar', 'gz'].includes(value)) return 'ZIP'
  return value ? value.slice(0, 4).toUpperCase() : 'FILE'
})
</script>

<template>
  <span class="jcr-file-icon" :class="{ 'jcr-file-icon--generic': generic }" :data-extension="extension" aria-hidden="true">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
      <path d="M6 2.75h7.25L18 7.5v13.75H6z" />
      <path d="M13 2.75V7.5h5" />
      <path v-if="generic" d="M8.75 12h6.5M8.75 15.5h6.5" />
    </svg>
    <b v-if="!generic">{{ extension }}</b>
  </span>
</template>

<style scoped>
.jcr-file-icon {
  position: relative;
  display: inline-flex;
  width: 38px;
  height: 42px;
  flex: 0 0 38px;
  align-items: center;
  justify-content: center;
  color: #64748b;
}
.jcr-file-icon svg { width: 34px; height: 40px; }
.jcr-file-icon b {
  position: absolute;
  right: 0;
  bottom: 5px;
  min-width: 28px;
  padding: 2px 3px;
  border-radius: 3px;
  background: #64748b;
  color: #fff;
  font-size: 8px;
  font-weight: 700;
  line-height: 1;
  letter-spacing: .02em;
  text-align: center;
}
.jcr-file-icon[data-extension="PDF"] { color: #e5484d; }
.jcr-file-icon[data-extension="PDF"] b { background: #e5484d; }
.jcr-file-icon[data-extension="DOC"] { color: #3976d9; }
.jcr-file-icon[data-extension="DOC"] b { background: #3976d9; }
.jcr-file-icon[data-extension="XLS"], .jcr-file-icon[data-extension="CSV"] { color: #269467; }
.jcr-file-icon[data-extension="XLS"] b, .jcr-file-icon[data-extension="CSV"] b { background: #269467; }
.jcr-file-icon[data-extension="PPT"] { color: #d5683a; }
.jcr-file-icon[data-extension="PPT"] b { background: #d5683a; }
.jcr-file-icon[data-extension="MD"], .jcr-file-icon[data-extension="TXT"] { color: #64748b; }
.jcr-file-icon[data-extension="MD"] b, .jcr-file-icon[data-extension="TXT"] b { background: #64748b; }
.jcr-file-icon--generic { width: 24px; height: 28px; flex-basis: 24px; color: currentColor; opacity: .78; }
.jcr-file-icon--generic svg { width: 23px; height: 27px; }
</style>

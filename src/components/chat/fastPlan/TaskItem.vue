<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { CheckCircle2, Loader2, AlertCircle, FileText, FileDown, Image as ImageIcon, X } from 'lucide-vue-next'
import MarkdownIt from 'markdown-it'
import type { ExecutionNodeState } from './session'

const FAST_PLAN_BASE = 'http://127.0.0.1:5179/FastPlan'

const props = defineProps<{
  task: ExecutionNodeState
  mode?: 'standalone' | 'embedded'
  forceCollapsed?: boolean
}>()

const emit = defineEmits<{ 'retry': [] }>()

const md = new MarkdownIt({ html: true, linkify: true, typographer: true, breaks: true })
const isExpanded = ref(true)
const showFullContent = ref(false)
const previewFile = ref<{ filename: string; url: string; original: string } | null>(null)
const showPreview = ref(false)
const contentRef = ref<HTMLElement | null>(null)

watch(() => props.forceCollapsed, (newVal) => { if (newVal) showFullContent.value = false })
watch(() => props.task?.status, (newVal) => { if (newVal === 'running') isExpanded.value = true })

const isRunning = computed(() => props.task.status === 'running')
const isCompleted = computed(() => props.task.status === 'completed')
const isFailed = computed(() => props.task.status === 'failed')
const isPending = computed(() => props.task.status === 'pending')

const statusConfig = computed(() => {
  if (isRunning.value) return { icon: Loader2, color: 'text-teal-500', bg: 'bg-teal-50', spin: true }
  if (isCompleted.value) return { icon: CheckCircle2, color: 'text-green-500', bg: 'bg-green-50', spin: false }
  if (isFailed.value) return { icon: AlertCircle, color: 'text-red-500', bg: 'bg-red-50', spin: false }
  return { icon: Loader2, color: 'text-gray-300', bg: 'bg-gray-50', spin: false }
})

const duration = computed(() => {
  if (!props.task.startTime) return null
  const end = props.task.endTime || Date.now()
  const ms = end - props.task.startTime
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
})

function getFileType(filename: string) {
  const ext = filename.split('.').pop()?.toLowerCase() || ''
  if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(ext)) return 'image'
  if (ext === 'pdf') return 'pdf'
  return 'other'
}

function getFileIcon(filename: string) {
  const type = getFileType(filename)
  if (type === 'image') return ImageIcon
  return FileText
}

function extractFiles(text: string) {
  if (!text) return []
  const regex = /(?:[\w\-.:\\]+[\\/])?[\w\-.一-龥]+\.(xlsx|xls|docx|doc|pdf|pptx|txt|csv|json|md|html|zip)/gi
  const matches = text.match(regex)
  if (!matches) return []
  return [...new Set(matches)].map(match => {
    const filename = match.split(/[\\/]/).pop() || match
    return { original: match, filename, url: `${FAST_PLAN_BASE}/api/files/${filename}` }
  })
}

const parsedJson = computed(() => {
  const content = props.task.output || props.task.streamContent || ''
  try {
    if (content.trim().startsWith('{')) {
      const obj = JSON.parse(content)
      if (obj && typeof obj === 'object') return obj
    }
  } catch (_) { /* ignore */ }
  return null
})

const generatedFiles = computed(() => {
  if (parsedJson.value) {
    const files: { filename: string; url: string; original: string }[] = []
    const downloadFiles = parsedJson.value.metadata?.download?.files
    if (Array.isArray(downloadFiles)) {
      downloadFiles.forEach((f: any) => {
        files.push({ filename: f.name, url: FAST_PLAN_BASE + (f.download_url || f.url), original: f.path })
      })
    } else if (Array.isArray(parsedJson.value.data)) {
      parsedJson.value.data.forEach((f: any) => {
        if (f.name && (f.url || f.download_url)) {
          files.push({ filename: f.name, url: FAST_PLAN_BASE + (f.download_url || f.url), original: f.url })
        }
      })
    }
    const seen = new Set<string>()
    const unique = files.filter(f => { if (seen.has(f.filename)) return false; seen.add(f.filename); return true })
    if (unique.length > 0) return unique
  }
  return extractFiles(props.task.output || props.task.streamContent || '')
})

const renderedContent = computed(() => {
  if (parsedJson.value) {
    if (parsedJson.value.message) {
      let msg: string = parsedJson.value.message
      if (generatedFiles.value.length > 0) {
        msg = msg.replace(/Download:.*$/i, '').trim()
        msg = msg.replace(/Task completed\.\s*\d+\s*file\(s\) generated\./i, '✅ 文件生成完成').trim()
      }
      return md.render(msg)
    }
    if (parsedJson.value.action === 'continue') {
      const text = parsedJson.value.value || '已确认继续'
      return `<div class="flex items-center gap-2 text-gray-600 bg-gray-50 px-3 py-2 rounded-lg border border-gray-100 inline-flex">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-green-500"><circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/></svg>
        <span>${text}</span></div>`
    }
    if (generatedFiles.value.length > 0) return ''
  }
  const content = props.task.streamContent || props.task.output || ''
  return content ? md.render(content) : ''
})

function openPreview(file: { filename: string; url: string; original: string }) {
  previewFile.value = file; showPreview.value = true
}

function closePreview() {
  showPreview.value = false; previewFile.value = null
}
</script>

<template>
  <div :class="['group transition-colors duration-150', mode === 'standalone' ? 'border-b last:border-0 border-gray-100 hover:bg-gray-50/50' : '']">
    <div v-if="mode === 'standalone'" class="p-4 flex items-start gap-3 cursor-pointer select-none" @click="isExpanded = !isExpanded">
      <div :class="['mt-0.5 flex-shrink-0 p-1.5 rounded-lg transition-colors', statusConfig.bg]">
        <component :is="statusConfig.icon" :class="['w-4 h-4', statusConfig.color, { 'animate-spin': statusConfig.spin }]" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="flex items-center justify-between gap-2">
          <h4 class="font-medium text-gray-900 truncate">{{ task.name || task.id }}</h4>
          <div class="flex items-center gap-3 flex-shrink-0">
            <span v-if="duration && (isCompleted || isRunning)" class="flex items-center gap-1 text-sm text-gray-400">
              <span class="text-xs">{{ duration }}</span>
            </span>
            <span :class="['text-sm font-medium px-2 py-0.5 rounded-full transition-all',
              isRunning ? 'bg-teal-100 text-teal-700' : '',
              isCompleted ? 'bg-green-100 text-green-700' : '',
              isFailed ? 'bg-red-100 text-red-700' : '',
              isPending ? 'bg-gray-100 text-gray-500' : '']">
              {{ isRunning ? '执行中' : isCompleted ? '完成' : isFailed ? '失败' : '等待' }}
            </span>
          </div>
        </div>
        <p v-if="!isExpanded && task.streamContent" class="text-sm text-gray-500 mt-1 truncate">
          {{ task.streamContent.substring(0, 80) }}{{ task.streamContent.length > 80 ? '...' : '' }}
        </p>
      </div>
    </div>

    <transition enter-active-class="transition-all duration-200 ease-out" leave-active-class="transition-all duration-150 ease-in"
      enter-from-class="opacity-0 max-h-0" enter-to-class="opacity-100 max-h-[2000px]"
      leave-from-class="opacity-100 max-h-[2000px]" leave-to-class="opacity-0 max-h-0">
      <div v-if="isExpanded || mode === 'embedded'" class="overflow-hidden">
        <div v-if="renderedContent" :class="mode === 'embedded' ? 'text-sm pt-2' : 'px-4 pb-4 pl-14'">
          <div class="prose prose-sm max-w-none text-gray-700 overflow-x-auto leading-relaxed"
            :class="{ 'typing-effect': isRunning, 'line-clamp-3 max-h-[4.5em] overflow-hidden': props.forceCollapsed && !showFullContent, 'max-h-[300px] overflow-y-auto custom-scrollbar': isRunning && mode === 'embedded' }"
            v-html="renderedContent" ref="contentRef" />
          <div v-if="props.forceCollapsed && !showFullContent" class="mt-2 text-center">
            <button @click.stop="showFullContent = true" class="text-sm text-teal-600 hover:text-teal-700 flex items-center justify-center gap-1 w-full py-1">
              展开详情
            </button>
          </div>
        </div>

        <div v-if="generatedFiles.length > 0" :class="mode === 'embedded' ? 'pt-2' : 'px-4 pb-4 pl-14'">
          <div class="flex flex-col gap-2">
            <div class="flex flex-wrap gap-3">
              <div v-for="file in generatedFiles" :key="file.filename"
                class="group/file relative flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-xl hover:border-teal-300 hover:shadow-md transition-all cursor-pointer min-w-[200px]"
                @click="openPreview(file)">
                <div class="w-10 h-10 rounded-lg bg-teal-50 flex items-center justify-center text-teal-600 flex-shrink-0">
                  <component :is="getFileIcon(file.filename)" :size="20" />
                </div>
                <div class="flex-1 min-w-0">
                  <div class="font-medium text-gray-700 truncate group-hover/file:text-teal-700">{{ file.filename }}</div>
                  <div class="text-[10px] text-gray-400 mt-0.5">点击预览</div>
                </div>
                <a :href="file.url" download @click.stop
                  class="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 rounded-full bg-gray-100 text-gray-500 opacity-0 group-hover/file:opacity-100 hover:bg-teal-100 hover:text-teal-600 transition-all"
                  title="下载文件">
                  <FileDown :size="14" />
                </a>
              </div>
            </div>
          </div>
        </div>

        <div v-if="task.error" :class="mode === 'embedded' ? 'pt-2' : 'px-4 pb-4 pl-14'">
          <div class="text-red-600 bg-red-50 p-3 rounded-lg border border-red-100">
            <div class="flex items-center justify-between mb-1">
              <div class="flex items-center gap-2 font-medium"><AlertCircle :size="14" /> 错误</div>
              <button @click.stop="$emit('retry')"
                class="flex items-center gap-1 px-2 py-1 bg-white border border-red-200 rounded text-sm text-red-600 hover:bg-red-50 transition-colors shadow-sm">
                重试
              </button>
            </div>
            <p class="text-sm break-all">{{ task.error }}</p>
          </div>
        </div>
      </div>
    </transition>

    <el-dialog v-model="showPreview" title="文件预览" width="50%" append-to-body :close-on-click-modal="false" destroy-on-close align-center @close="closePreview">
      <div class="w-full max-h-[90vh] overflow-hidden flex flex-col relative">
        <div class="flex items-center justify-between p-4 border-b border-gray-100">
          <div class="flex items-center gap-3">
            <div class="w-8 h-8 rounded-lg bg-teal-50 flex items-center justify-center text-teal-600">
              <component :is="previewFile ? getFileIcon(previewFile.filename) : FileText" :size="18" />
            </div>
            <div>
              <h3 class="font-bold text-gray-900">{{ previewFile?.filename }}</h3>
              <p class="text-sm text-gray-500">文件预览</p>
            </div>
          </div>
          <a v-if="previewFile" :href="previewFile.url" download
            class="px-3 py-1.5 bg-teal-50 text-teal-700 rounded-lg hover:bg-teal-100 text-sm font-medium flex items-center gap-1 transition-colors">
            <FileDown :size="14" /> 下载
          </a>
        </div>
        <div class="flex-1 overflow-auto bg-gray-50 p-4 flex items-center justify-center min-h-[300px]">
          <img v-if="previewFile && getFileType(previewFile.filename) === 'image'" :src="previewFile.url"
            class="max-w-full max-h-full object-contain shadow-lg rounded-lg" />
          <iframe v-else-if="previewFile && getFileType(previewFile.filename) === 'pdf'" :src="previewFile.url"
            class="w-full h-full min-h-[500px] rounded-lg border border-gray-200"></iframe>
          <div v-else class="text-center">
            <div class="w-20 h-20 bg-gray-200 rounded-full flex items-center justify-center mx-auto mb-4 text-gray-400">
              <FileText :size="40" />
            </div>
            <h4 class="text-gray-900 font-medium mb-2">暂不支持在线预览此格式</h4>
            <p class="text-gray-500 mb-6 max-w-xs mx-auto">该文件需要下载后使用本地应用打开。</p>
            <a v-if="previewFile" :href="previewFile.url" download
              class="px-6 py-2.5 bg-teal-600 text-white rounded-xl hover:bg-teal-700 font-medium shadow-lg inline-flex items-center gap-2">
              <FileDown :size="18" /> 下载文件
            </a>
          </div>
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<style>
.prose h1, .prose h2, .prose h3 { margin-top: 1em; margin-bottom: 0.5em; font-weight: 600; color: #1f2937; }
.prose h1 { font-size: 1.5em; }
.prose h2 { font-size: 1.25em; border-bottom: 1px solid #e5e7eb; padding-bottom: 0.3em; }
.prose h3 { font-size: 1.1em; }
.prose p { margin-bottom: 0.8em; line-height: 1.7; }
.prose ul, .prose ol { padding-left: 1.5em; margin-bottom: 0.8em; }
.prose li { margin-bottom: 0.3em; }
.prose code { background-color: #f3f4f6; padding: 0.15em 0.4em; border-radius: 0.25em; font-size: 0.9em; color: #ea580c; }
.prose pre { background-color: #1e293b; color: #f8fafc; padding: 1em; border-radius: 0.5em; overflow-x: auto; margin-bottom: 1em; }
.prose pre code { background-color: transparent; color: inherit; padding: 0; }
.prose blockquote { border-left: 3px solid #14b8a6; padding-left: 1em; margin: 1em 0; color: #6b7280; font-style: italic; }
.prose table { width: 100%; border-collapse: collapse; margin: 1em 0; }
.prose th, .prose td { border: 1px solid #e5e7eb; padding: 0.5em 0.75em; text-align: left; }
.prose th { background-color: #f9fafb; font-weight: 600; }
.typing-effect > *:last-child::after { content: ''; display: inline-block; width: 2px; height: 1.2em; background-color: #14b8a6; margin-left: 2px; vertical-align: text-bottom; animation: blink 1s step-end infinite; }
@keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }
.custom-scrollbar::-webkit-scrollbar { width: 4px; }
.custom-scrollbar::-webkit-scrollbar-thumb { background-color: #cbd5e1; border-radius: 2px; }
</style>

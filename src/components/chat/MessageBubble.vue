<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, onBeforeUnmount, onActivated, onDeactivated, inject, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'

import { CHAT_MESSAGE_EXPORT } from '@/modules/chat/messageExportBinding'
import { useMessageDeleteConfirmation } from '@/modules/chat/messageDeleteConfirmation'
import type { Message, ChatAttachment, ToolCallEvent } from '@/types'
import MarkdownViewer from '@/components/markdown/MarkdownViewer.vue'
import ToolCallCard from '@/components/chat/ToolCallCard.vue'
import GroundingPanel from '@/components/chat/GroundingPanel.vue'
import SvgIcon from '@/components/svgIcon/index.vue'
import ModelSelector from '@/components/chat/ModelSelector.vue'
import personAvatar from '@/assets/sex/person.png'
import logoSvg from '@/assets/logo.svg?raw'
import { chatModelIcon } from '@/api/branding'

interface ModelGroupItem {
  id: string;
  name: string;
  rawModelId: string;
  modelId: string,
  tags: Array<{
    value: string,
    label: string
  }>,
  modelLabel: string,
}
interface ModelGroup {
  id: string,
  label: string,
  children: ModelGroupItem[],
  providerCode?: string,
  sourceCategory?: string,
}

function onUserAvatarError(e: Event) {
  const img = e.target as HTMLImageElement
  img.src = personAvatar
}

const props = defineProps<{
  message: Message
  showSwitchModel?: boolean
  showRegenerate?: boolean
  showBranch?: boolean
  modelGroups?: ModelGroup[]
  disableEdit?: boolean
  userAvatarSrc?: string
  readingId?: string | null  // 当前正在朗读的消息 ID（null 表示无朗读）
}>()

/** 将 LLM API 错误拆成多行，并把响应 JSON 格式化展示 */
const formattedError = computed(() => {
  const raw = props.message.content || ''

  const responseMatch = raw.match(/响应:\s*(\{[\s\S]*\})\s*$/)
  const jsonMatch = responseMatch?.[1] || raw.match(/(\{[\s\S]*\})/)?.[1]
  let pretty = ''
  if (jsonMatch) {
    try {
      pretty = JSON.stringify(JSON.parse(jsonMatch), null, 2)
    } catch {
      pretty = jsonMatch
    }
  }

  let text = jsonMatch
    ? raw.replace(jsonMatch, '').replace(/\s*响应:\s*$/u, '响应:').trimEnd()
    : raw

  // 旧单行：LLM API 错误 [模型: x, 供应商: y (url)]: HTTP 503, 响应:
  text = text.replace(
    /LLM API 错误\s*\[\s*模型:\s*([^,\]]+?)\s*,\s*供应商:\s*([^\]]+?)\s*\]\s*:\s*HTTP\s*(\d+)\s*,?\s*/u,
    'LLM API 错误\n模型: $1\n供应商：$2\nHTTP状态码:$3\n',
  )

  // 去掉「模型: xxx,」行尾逗号
  text = text.replace(/(模型:\s*[^\n,]+),(\s*\n)/u, '$1$2')

  // 统一「响应:」单独成行
  if (/响应:\s*$/u.test(text) && !/\n响应:\s*$/u.test(text)) {
    text = text.replace(/\s*响应:\s*$/u, '\n响应:')
  }

  return { text, json: pretty }
})

const emit = defineEmits<{
  'copy': [content: string]
  'regenerate': [id: string]
  'delete': [id: string]
  'edit': [id: string]
  'translate': [id: string, lang: string]
  'tts': [id: string, content: string]
  'favorite': [id: string, next: boolean]
  'quote': [content: string]
  'edit-as-doc': [content: string]
  'switch-model': [id: string]
  'switch-model-change': [messageId: string, modelId: string]
  'like': [id: string, next: number]
  'dislike': [id: string, next: number]
  'trace-source': [type: string, messageId: string]
  'branch': [id: string]
}>()

const { t } = useI18n()
const copied = ref(false)
const copiedUser = ref(false)
const deleteConfirmation = useMessageDeleteConfirmation(() => props.message, id => emit('delete', id))

const favorited = computed(() => props.message.is_favorite === 1)
const thinkShow = ref(true)
const isLiked = computed(() => props.message.is_liked === 1)
const isDisliked = computed(() => props.message.is_liked === -1)
const getProviderIcon = inject('getProviderIcon') as (providerCode: string, sourceCategory: string) => string
const isVendorIncomplete = inject('isVendorIncomplete', () => false) as (providerCode: string) => boolean

const VENDOR_PREFIXES = ['HUIZHI-', 'PRIVATE-', 'LOCAL-', 'HUIZHI:', 'PRIVATE:', 'LOCAL:', 'HUIZHI/', 'PRIVATE/', 'LOCAL/', 'HUIZHI_', 'PRIVATE_', 'LOCAL_']

function formatModelName(m: ModelGroupItem): string {
  return m.name
}

function findModelById(id: string): ModelGroupItem | undefined {
  for (const g of (props.modelGroups || [])) {
    // 精确 ID 匹配：支持 modelId / rawModelId / id 任意一种
    const m = g.children.find(x =>
      String(x.modelId || x.id) === id ||
      String(x.rawModelId) === id ||
      String(x.id) === id ||
      x.name === id
    )
    if (m) return m
  }
  return undefined
}

const modelDisplayName = computed(() => {
  const mid = props.message.model_id
  if (!mid) return ''
  let midStr = String(mid)

  // 先去掉供应商前缀，再查找模型（支持 HUIZHI-/PRIVATE-/LOCAL- 等格式）
  for (const prefix of VENDOR_PREFIXES) {
    if (midStr.toUpperCase().startsWith(prefix)) {
      midStr = midStr.slice(prefix.length)
      break
    }
  }
  // 兜底：去掉任意 "大写字母序列-" 前缀
  midStr = midStr.replace(/^[A-Z]+[-:]/g, '')

  const m = findModelById(midStr)
  if (m) return formatModelName(m)

  return midStr
})

function parseTimestamp(t: string): Date {
  // ISO 8601 with explicit timezone offset: parse directly (e.g. "2026-07-01T14:30:00+08:00")
  if (t.includes('+') || t.includes('Z')) return new Date(t)
  // ISO 8601 without timezone: treat as UTC
  if (t.includes('T')) return new Date(t + 'Z')
  // Old format or backend returns without timezone (e.g. "2026-07-01 14:30:00"):
  // Backend stores in CST (+08:00), so append explicit CST offset
  if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/.test(t)) {
    return new Date(t + '+08:00')
  }
  // Fallback
  return new Date(t)
}

// 周期性刷新时间显示（修复 #229 时间不更新）
const now = ref(Date.now())
let timeTimer: ReturnType<typeof setInterval> | null = null
onMounted(() => { timeTimer = setInterval(() => { now.value = Date.now() }, 30000) })
onUnmounted(() => { if (timeTimer) clearInterval(timeTimer) })

const formattedTime = computed(() => {
  const ts = props.message.created_at
  if (!ts) return ''
  const d = parseTimestamp(ts)
  return d.toLocaleDateString(t('chat.locale'), { month: 'long', day: 'numeric', hour: '2-digit', minute: '2-digit' })
})

// 用户消息中的图片附件（用于气泡内缩略图展示）
const imageAttachments = computed<string>(() => {
  if (props.message.role !== 'user' || !props.message.attachments) return ''
  return props.message.attachments.filter(
    (a) => a.type === 'image' || a.type === 'screenshot',
  ).map((i, e) => {
    return `![${e + 1}](${i.preview_url})`
  }).join(' ')
})

function toggleThink() {
  thinkShow.value = !thinkShow.value
}

const handleCopy = async () => {
  try {
    await navigator.clipboard.writeText(props.message.content)
    copied.value = true
    ElMessage.success(t('chat.copied'))
    setTimeout(() => { copied.value = false }, 1500)
  } catch { /* fallback */ }
}

const handleCopyUser = async () => {
  try {
    await navigator.clipboard.writeText(props.message.content)
    copiedUser.value = true
    ElMessage.success(t('chat.copied'))
    setTimeout(() => { copiedUser.value = false }, 1500)
  } catch { /* fallback */ }
}

const handleFavorite = () => {
  const next = !favorited.value
  emit('favorite', props.message.id, next)
}

const translatePopoverRef = ref()
const tracePopoverRef = ref()
const exportPopoverRef = ref()
const switchModelActive = ref(false)
const switchModelPendingId = ref(props.message.model_id || '')

const switchModelSelectorRef = ref()

function onLocalSwitchModel(id: string | undefined) {
  if (id && id !== props.message.model_id) {
    emit('switch-model-change', props.message.id, id)
  }
  switchModelPendingId.value = props.message.model_id || ''
  switchModelActive.value = false
}

function openSwitchModelDropdown() {
  switchModelPendingId.value = props.message.model_id || ''
  switchModelActive.value = true
  nextTick(() => {
    const elSelect = switchModelSelectorRef.value?.modelSelector as any
    if (!elSelect) return
    if (typeof elSelect.toggleMenu === 'function') {
      elSelect.toggleMenu()
      return
    }
    elSelect.focus()
    nextTick(() => {
      const el = elSelect.$el as HTMLElement | null
      const wrapper = el?.querySelector('.el-input__wrapper, .el-select__wrapper') as HTMLElement | null
      ;(wrapper || el)?.click()
    })
  })
}

function handleTranslate(lang: string) {
  emit('translate', props.message.id, lang)
  translatePopoverRef.value?.hide()
}

function handleTraceSource(type: string, messageId: string) {
  emit('trace-source', type, messageId)
  tracePopoverRef.value?.hide()
}

const exporter = inject(CHAT_MESSAGE_EXPORT, null)
const exportActive = ref(true), exportGeneration = ref(0)
onActivated(() => { exportActive.value = true })
onDeactivated(() => { exportActive.value = false; exportGeneration.value++ })
onBeforeUnmount(() => { exportActive.value = false; exportGeneration.value++ })
const exportMenu = computed(() => {
  const message = props.message, content = message.content, generation = exportGeneration.value
  const current = () => exportActive.value && exportGeneration.value === generation && props.message === message && message.content === content
  const run = exporter?.captureExport(message.id, current)
  const select = (format: string) => async () => {
    if (!current() || !run) return
    exportPopoverRef.value?.hide()
    await run(format)
  }
  return { md: select('md'), docx: select('docx'), pdf: select('pdf') }
})
</script>

<template>
  <div class="message-wrapper" :class="[message.role]">
    <div class="message" :class="[message.role, message.status]">
      <!-- AI 头像：优先使用自定义对话模型图标，否则用默认 Logo -->
      <div v-if="message.role === 'assistant'" class="avatar">
        <img v-if="chatModelIcon" :src="chatModelIcon" alt="AI" class="avatar-img" />
        <span v-else v-html="logoSvg" class="avatar-img avatar-svg" />
      </div>

      <!-- 用户头像 -->
      <div v-if="message.role === 'user'" class="avatar user-avatar">
        <img :src="props.userAvatarSrc" alt="User" class="avatar-img" @error="onUserAvatarError" />
      </div>

      <div class="bubble-container">
        <!-- 用户消息 -->
        <div v-if="message.role === 'user'" class="bubble user-bubble">
          <div class="content user-content">{{ message.content }}</div>
          <!-- Skill 调用标签（用户选择了 Skill 时显示） -->
          <div v-if="message.skill_used" class="skill-used-badge">
            <span class="skill-badge-icon">🔧</span>
            <span class="skill-badge-label">Skill</span>
            <span class="skill-badge-name">{{ message.skill_used.name }}</span>
          </div>
          <!-- 用户上传的图片缩略图 -->
          <div v-if="imageAttachments.length" class="user-images">
            <!-- <div v-for="img in imageAttachments" :key="img.id" class="user-image-thumb">
              <img :src="img.preview_url" :alt="img.name" class="user-image-preview" />
            </div> -->
            <MarkdownViewer :content="imageAttachments || ''" custom-class="custom-show-img" />
          </div>

          <span v-if="message.is_edited" class="edited-mark">{{ t('chat.edited') }}</span>
          <div class="user-actions">
            <button class="action-btn" :class="{ active: copiedUser }" @click="handleCopyUser"
              :title="t('common.copy')">
              <svg v-if="!copiedUser" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2"
                fill="none">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
              </svg>
              <svg v-else viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                <polyline points="20 6 9 17 4 12"></polyline>
              </svg>
            </button>
            <button class="action-btn" :disabled="props.disableEdit" :title="$t('common.edit')"
              @click="emit('edit', message.id)">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
              </svg>
            </button>
          </div>
        </div>

        <!-- AI 消息：Markdown 渲染 -->
        <div v-else class="bubble ai-bubble">
          <!-- 模型名称 + 时间 -->
          <div class="msg-header">
            <span class="msg-model-name">{{ modelDisplayName }}</span>
            <span class="msg-time">{{ formattedTime }}</span>
          </div>
          <!-- Skill 调用标签 -->
          <div v-if="message.skill_used" class="skill-used-badge">
            <span class="skill-badge-icon">🔧</span>
            <span class="skill-badge-label">Skill</span>
            <span class="skill-badge-name">{{ message.skill_used.name }}</span>
            <span v-if="message.skill_used.args" class="skill-badge-args">{{ message.skill_used.args }}</span>
          </div>

          <!-- 思考过程（可折叠） -->
          <div v-if="message.think_content?.trim()" class="think-section">
            <button class="think-toggle" @mousedown="toggleThink">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"
                class="think-arrow" :class="{ expanded: thinkShow }">
                <polyline points="9 18 15 12 9 6" />
              </svg>
              <span>Thinking{{ message.status === 'streaming' && message.stream_phase === 'reasoning' && !thinkShow ? '...' : '' }}</span>
            </button>
            <div v-if="thinkShow" class="think-content">
              <MarkdownViewer :content="message.think_content || ''" />
            </div>
          </div>

          <!-- 工具调用卡片（running→done/error，持久不被冲掉） -->
          <div v-if="(message.tool_calls || []).length" class="tool-calls-section">
            <ToolCallCard
              v-for="tc in message.tool_calls"
              :key="tc.id"
              :tool="tc"
            />
          </div>

          <!-- 等待响应与真实思考内容分别展示 -->
          <div v-if="message.isThinking && message.status === 'streaming' && !message.content && !message.think_content?.trim()" class="thinking-skeleton">
            <span class="thinking-spinner"></span>
            <span class="thinking-text">{{ message.stream_phase === 'finalizing' ? '正在生成最终回答…' : '等待模型响应…' }}</span>
          </div>

          <template v-if="message.status === 'error'">
            <div class="content error-content">
              <div class="error-text">{{ formattedError.text }}</div>
              <pre v-if="formattedError.json" class="error-json">{{ formattedError.json }}</pre>
            </div>
          </template>
          <template v-else>
            <div class="content">
              <MarkdownViewer :content="message.content" />
            </div>
            <!-- 流式输出光标 -->
            <!-- <span v-if="message.status === 'streaming'" class="stream-cursor">▍</span> -->
            <!-- 证据台账：回答里的数字有无出处 + 系统提示语（后者不经模型） -->
            <GroundingPanel :grounding="message.grounding" :note="message.grounding_note" />
          </template>

          <!-- 翻译结果（分隔线 + 译文） -->
          <template v-if="message.translation">
            <div class="translate-divider">
              <div class="translate-divider-line"></div>
              <span class="translate-divider-icon">⇄</span>
            </div>
            <div class="translation-section">
              <div class="translation-label">{{ message.translation_lang || 'en' }}:</div>
              <MarkdownViewer :content="message.translation" />
            </div>
          </template>
        </div>

        <!-- 消息内容与操作栏之间的插槽（如 doc-edit 版本卡片） -->
        <div class="below-content-slot">
          <slot name="below-content" />
        </div>

        <!-- Token 信息 -->
        <div v-if="message.token_input && message.role !== 'user'" class="meta">
          <span class="model-tag">{{ modelDisplayName }}</span>
          <span>输入 {{ message.token_input }}、输出 {{ message.token_output }} tokens</span>
        </div>

        <!-- 操作按钮栏（AI 回复） -->
        <div v-if="message.role === 'assistant' && (message.status === 'completed' || message.status === 'canceled')"
          class="action-bar">
          <button  class="action-btn" :class="{ active: copied }" @click="handleCopy"
            :title="t('common.copy')">
            <svg v-if="!copied" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2"
              fill="none">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
            </svg>
            <svg v-else viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          </button>
          <button v-if="showRegenerate !== false" class="action-btn" @click="emit('regenerate', message.id)"
            :title="t('chat.regenerate')">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
              <polyline points="23 4 23 10 17 10"></polyline>
              <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"></path>
            </svg>
          </button>
          <button  class="action-btn" @click="emit('edit', message.id)"
            :title="t('common.edit')">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
            </svg>
          </button>
          <!-- 翻译下拉 -->
          <el-popover v-if="!message.translating" placement="top" :width="150"
            trigger="click" ref="translatePopoverRef">
            <template #reference>
              <button class="action-btn" :class="{ active: !!message.translation }" :title="t('chat.translate')">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <path d="M5 8l6 6" />
                  <path d="M4 14l6-6 2-3" />
                  <path d="M2 5h12" />
                  <path d="M7 2h1" />
                  <path d="m22 22-5-10-5 10" />
                  <path d="M14 18h6" />
                </svg>
              </button>
            </template>
            <div class="translate-menu">
              <button class="dropdown-item" @click="handleTranslate('zh')">中文</button>
              <button class="dropdown-item" @click="handleTranslate('zh-TW')">繁體中文</button>
              <button class="dropdown-item" @click="handleTranslate('en')">English</button>
              <button class="dropdown-item" @click="handleTranslate('ja')">日本語</button>
              <button class="dropdown-item" @click="handleTranslate('ko')">한국어</button>
              <button class="dropdown-item" @click="handleTranslate('fr')">Français</button>
              <button class="dropdown-item" @click="handleTranslate('es')">Español</button>
            </div>
          </el-popover>
          <!-- 翻译中 loading（不可点击） -->
          <button v-if="message.translating" class="action-btn translating-btn" disabled
            :title="t('chat.translating')">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"
              class="spin-icon">
              <circle cx="12" cy="12" r="10" />
              <path d="M12 6v6l4 2" />
            </svg>
            <span class="translating-label">{{ t('chat.translating') }}</span>
          </button>
          <button class="action-btn" :class="{ active: favorited }" @click="handleFavorite" :title="t('chat.favorite')">
            <svg viewBox="0 0 24 24" width="14" height="14" :stroke="favorited ? '#faad14' : 'currentColor'"
              stroke-width="2" :fill="favorited ? '#faad14' : 'none'">
              <polygon
                points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2">
              </polygon>
            </svg>
          </button>
          <button  class="action-btn" @click="emit('edit-as-doc', message.content)"
            :title="t('chat.editAsDoc')">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
              <polyline points="14 2 14 8 20 8"></polyline>
              <line x1="16" y1="13" x2="8" y2="13"></line>
              <line x1="16" y1="17" x2="8" y2="17"></line>
            </svg>
          </button>
          <div v-if="showSwitchModel" class="switch-model-wrap">
            <button class="action-btn" :title="t('chat.switchModelTitle')" @click="openSwitchModelDropdown">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                <polyline points="16 3 21 3 21 8"></polyline>
                <line x1="4" y1="20" x2="21" y2="3"></line>
                <polyline points="21 16 21 21 16 21"></polyline>
                <line x1="15" y1="15" x2="21" y2="21"></line>
                <line x1="4" y1="4" x2="9" y2="9"></line>
              </svg>
            </button>
            <ModelSelector
              v-if="switchModelActive"
              ref="switchModelSelectorRef"
              v-model="switchModelPendingId"
              :model-groups="modelGroups"
              hide-advanced
              show-incomplete-filter
              :is-vendor-incomplete="isVendorIncomplete"
              icon-tags
              class="switch-model-hidden"
              @change="onLocalSwitchModel"
              @visible-change="(v) => { if (!v) { switchModelPendingId = props.message.model_id || ''; switchModelActive = false } }"
            />
          </div>
          <button  class="action-btn" @click="emit('quote', message.content)"
            :title="t('chat.followUp')">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
            </svg>
          </button>
          <button  class="action-btn"
            :class="{ active: readingId && readingId === message.id }"
            @click="emit('tts', message.id, message.content)"
            :title="readingId && readingId === message.id ? t('chat.stopReading') : t('chat.readAloud')">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
              <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon>
              <path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path>
            </svg>
          </button>
          <!-- 溯源 -->
          <el-popover placement="top" :width="160" trigger="click" v-if="message.trace_source" ref="tracePopoverRef">
            <template #reference>
              <button class="action-btn" :title="t('chat.traceTitle')">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <circle cx="11" cy="11" r="8" />
                  <line x1="21" y1="21" x2="16.65" y2="16.65" />
                </svg>
              </button>
            </template>
            <div class="translate-menu">
              <button v-if="message.trace_source?.local" class="dropdown-item"
                @click="handleTraceSource('local', message.id)">{{ t('chat.traceLocal') }}</button>
              <button v-if="message.trace_source?.graph" class="dropdown-item"
                @click="handleTraceSource('graph', message.id)">{{ t('chat.traceGraph') }}</button>
              <button v-if="message.trace_source?.file" class="dropdown-item"
                @click="handleTraceSource('file', message.id)">{{
                  t('chat.traceFile') }}</button>
              <button class="dropdown-item" @click="handleTraceSource('result', message.id)">{{ t('chat.traceResult')
              }}</button>
              <button class="dropdown-item" @click="handleTraceSource('analysis', message.id)">{{
                t('chat.traceAnalysis')
              }}</button>
            </div>
          </el-popover>

          <el-popover placement="top" :width="130" trigger="click" ref="exportPopoverRef">
            <template #reference>
              <button class="action-btn" :title="t('chat.exportTitle')">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                  <polyline points="7 10 12 15 17 10"></polyline>
                  <line x1="12" y1="15" x2="12" y2="3"></line>
                </svg>
              </button>
            </template>
            <div class="translate-menu">
              <button class="dropdown-item" @click="exportMenu.md">{{ t('chat.exportMD') }}</button>
              <button class="dropdown-item" @click="exportMenu.docx">{{ t('chat.exportDOCX') }}</button>
              <button class="dropdown-item" @click="exportMenu.pdf">{{ t('chat.exportPDF') }}</button>
            </div>
          </el-popover>
          <div class="action-divider"></div>
          <button class="action-btn" :class="{ active: isLiked }" @click="emit('like', message.id, isLiked ? 0 : 1)"
            :title="t('chat.like')">
            <svg viewBox="0 0 24 24" width="14" height="14" :stroke="isLiked ? 'none' : 'currentColor'" stroke-width="2"
              :fill="isLiked ? 'var(--color-primary)' : 'none'">
              <path
                d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3H14zM7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3">
              </path>
            </svg>
          </button>
          <button class="action-btn" :class="{ active: isDisliked }"
            @click="emit('dislike', message.id, isDisliked ? 0 : -1)" :title="t('chat.dislike')">
            <svg viewBox="0 0 24 24" width="14" height="14" :stroke="isDisliked ? 'none' : 'currentColor'"
              stroke-width="2" :fill="isDisliked ? 'var(--color-danger)' : 'none'">
              <path
                d="M10 15v4a3 3 0 0 0 3 3l4-9V2H5.72a2 2 0 0 0-2 1.7l-1.38 9a2 2 0 0 0 2 2.3H10zM17 2h2.67a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H17">
              </path>
            </svg>
          </button>
          <button v-if="showBranch !== false" class="action-btn" @click="emit('branch', message.id)"
            :title="t('chat.createBranch')">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
              <line x1="6" y1="3" x2="6" y2="15" />
              <circle cx="18" cy="6" r="3" />
              <circle cx="6" cy="21" r="3" />
              <path d="M18 9a9 9 0 0 1-9 9" />
            </svg>
          </button>
          <div class="action-divider"></div>
          <el-popconfirm :title="t('chat.deleteMessageConfirm')" :confirm-button-text="t('common.delete')"
            :cancel-button-text="t('common.cancel')" @confirm="deleteConfirmation.confirm" @hide="deleteConfirmation.cancel">
            <template #reference>
              <button class="action-btn action-btn--danger" :title="t('common.delete')" @click="deleteConfirmation.open">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <polyline points="3 6 5 6 21 6"></polyline>
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                </svg>
              </button>
            </template>
          </el-popconfirm>
        </div>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.message-wrapper {
  display: flex;
  margin-bottom: 28px;
  width: 100%;

  &.user {
    justify-content: flex-end;
    margin-bottom: 10px;
  }

  &.export-select-mode {
    cursor: pointer;
  }
}

/* 导出多选框 */
.export-check-cell {
  display: flex;
  align-items: flex-start;
  padding: 12px 8px 0 0;
  flex-shrink: 0;
  cursor: pointer;
}

.message {
  display: flex;
  gap: 12px;
  max-width: 100%;

  &.user {
    flex-direction: row-reverse;

    .bubble {
      // background: var(--theme-question-bubble);
      background: transparent !important;
      color: var(--text-primary);
      //border-radius: 18px 4px 18px 18px;
      border-radius: 8px 8px 8px 8px;
      padding: 12px 16px 6px;
    }
  }

  &.assistant {
    .bubble {
      // background: var(--ai-answer-background); //回答区域背景颜色
      color: var(--text-primary);
      padding: 0;
      margin: 15px;
    }
  }
}

.avatar {
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  overflow: hidden;
  margin-top: 2px;
}

.user-avatar {
  border-radius: 50%;
}

.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.avatar-svg {
  color: var(--color-primary);

  :deep(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
}

.bubble-container {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  background: var(--ai-answer-background) !important;
  border-radius: 8px !important;
  .below-content-slot {
    margin-left: 10px;
  }
}


.bubble {
  border-radius: 12px;

  .user-content {
    font-size: var(--font-size-base);
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-word;
    padding: 0;
  }

  /* ─── 用户上传图片缩略图 ─── */
  .user-images {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;

    .custom-show-img {
      padding: 0;

      & :deep(img) {
        max-width: 120px !important;
        max-height: 120px !important;
        object-fit: contain;
        display: inline-block;
      }
    }
  }

  .user-image-thumb {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    max-width: 120px;
  }

  .user-image-preview {
    width: 120px;
    height: 120px;
    object-fit: cover;
    border-radius: 10px;
    border: 1px solid var(--border-color);
    cursor: pointer;
    transition: transform 0.15s, box-shadow 0.15s;

    &:hover {
      transform: scale(1.05);
      box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    }
  }

  .user-image-name {
    font-size: 11px;
    color: var(--text-tertiary);
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

/* ====== Markdown 渲染样式 ====== */
.markdown-body {
  font-size: var(--font-size-base);
  line-height: 1.75;
  word-break: break-word;

  :deep(h1),
  :deep(h2),
  :deep(h3),
  :deep(h4) {
    margin: 16px 0 8px;
    font-weight: 600;
    line-height: 1.4;
    color: var(--text-primary) !important;
  }

  :deep(h1) {
    font-size: 1.4em;
  }

  :deep(h2) {
    font-size: 1.25em;
  }

  :deep(h3) {
    font-size: 1.1em;
  }

  :deep(p) {
    margin: 8px 0;
  }

  :deep(ul),
  :deep(ol) {
    padding-left: 1.5em;
    margin: 8px 0;
  }

  :deep(li) {
    margin: 4px 0;
  }

  :deep(blockquote) {
    border-left: 3px solid var(--color-primary);
    margin: 12px 0;
    padding: 8px 16px;
    color: var(--text-secondary);
    background: rgba(24, 144, 255, 0.04);
    border-radius: 0 8px 8px 0;
  }

  :deep(a) {
    color: var(--color-primary);
    text-decoration: none;

    &:hover {
      text-decoration: underline;
    }
  }

  :deep(hr) {
    border: none;
    border-top: 1px solid var(--chat-divider-color);
    margin: 16px 0;
  }

  :deep(img) {
    max-width: 100%;
    border-radius: 8px;
    margin: 8px 0;
  }

  /* 行内代码 */
  :deep(code:not(.hljs)) {
    background: rgba(0, 0, 0, 0.05);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.9em;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Consolas, monospace;
    color: #c7254e;
  }

  /* 代码块 */
  :deep(.code-block) {
    margin: 12px 0;
    border-radius: 10px;
    overflow: hidden;
    background: #1e1e2e;
    font-size: var(--font-size-md);
    line-height: 1.6;

    .code-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 8px 16px;
      background: rgba(255, 255, 255, 0.06);
      border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }

    .code-lang {
      font-size: var(--font-size-sm);
      color: rgba(255, 255, 255, 0.5);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .code-copy-btn {
      font-size: var(--font-size-sm);
      color: rgba(255, 255, 255, 0.5);
      background: none;
      border: none;
      cursor: pointer;
      padding: 2px 8px;
      border-radius: 4px;
      transition: all 0.2s;

      &:hover {
        color: rgba(255, 255, 255, 0.8);
        background: rgba(255, 255, 255, 0.1);
      }
    }

    code {
      display: block;
      padding: 16px;
      overflow-x: auto;
      color: #0505be;
      font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Consolas, monospace;
    }
  }

  /* KaTeX */
  :deep(.katex-block) {
    margin: 16px 0;
    overflow-x: auto;
    padding: 12px 0;
    text-align: center;
  }

  :deep(.katex) {
    font-size: 1.05em;
  }

  :deep(.katex-error) {
    color: var(--color-danger);
    font-family: monospace;
    font-size: var(--font-size-md);
  }

  /* 表格 */
  :deep(.md-table-wrapper) {
    overflow-x: auto;
    margin: 12px 0;
    border-radius: 8px;
    border: 1px solid var(--border-color);
  }

  :deep(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-base);
  }

  :deep(th) {
    background: rgba(0, 0, 0, 0.03);
    font-weight: 600;
    text-align: left;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border-color);
  }

  :deep(td) {
    padding: 8px 14px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.04);
  }

  :deep(tr:last-child td) {
    border-bottom: none;
  }

  /* Mermaid */
  :deep(.mermaid-block) {
    margin: 12px 0;
    background: rgba(0, 0, 0, 0.02);
    border-radius: 8px;
    padding: 16px;
    overflow-x: auto;
  }

  /* 强制 md-editor 文字跟随主题色 */
  :deep(.md-editor),
  :deep(.md-editor-preview-wrapper),
  :deep(.md-editor-preview-content) {
    color: var(--text-primary) !important;
  }

  :deep(.md-editor p),
  :deep(.md-editor li),
  :deep(.md-editor h1),
  :deep(.md-editor h2),
  :deep(.md-editor h3),
  :deep(.md-editor h4),
  :deep(.md-editor h5),
  :deep(.md-editor h6),
  :deep(.md-editor blockquote),
  :deep(.md-editor table) {
    color: var(--text-primary) !important;
  }
}

/* 流式光标 */
.stream-cursor {
  display: inline-block;
  animation: blink 1s step-end infinite;
  color: var(--color-primary);
  font-weight: 300;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}


.error-content {
  // color: var(--color-danger);
  color:var(--text-color-danger);
  font-size: var(--font-size-base);
  line-height: 1.6;
  padding: 12px 16px;
  // background: rgba(255, 77, 79, 0.06);
  border-radius: 10px;
  border: 1px solid rgba(255, 77, 79, 0.15);
}

.error-text {
  white-space: pre-wrap;
  word-break: break-word;
}

.error-json {
  // margin: 10px 0 0;
  // padding: 10px 12px;
  // border-radius: 8px;
  // background: rgba(0, 0, 0, 0.06);
  // border: 1px solid rgba(255, 77, 79, 0.18);
  // color: var(--color-danger);
  font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
  font-size: var(--font-size-sm, 12px);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-x: auto;
}

/* ====== 模型名称 + 时间 ====== */
.msg-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 6px;
  background: transparent !important;

  .msg-model-name {
    font-size:  var(--font-size-lg);
    font-weight: 600;
    color: var(--text-primary);
  }

  .msg-time {
    font-size:  var(--font-size-base);
    color: var(--text-tertiary);
  }
}

/* ====== Think 思考过程折叠 ====== */
.think-section {
  margin-bottom: 8px;
  border: 1px solid var(--chat-divider-color);
  border-radius: 10px;
  overflow: hidden;
  //background: color-mix(in srgb, var(--bg-secondary, #f9fafb) 60%, transparent);
}

.think-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 8px 14px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--font-size-md);
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: rgba(0, 0, 0, 0.03);
    color: var(--text-primary);
  }

  .think-arrow {
    transition: transform 0.2s;
    flex-shrink: 0;

    &.expanded {
      transform: rotate(90deg);
    }
  }
}

.think-content {
  padding: 8px 14px 14px;
  border-top: 1px solid var(--border-color);
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  max-height: 300px;
  overflow-y: auto;

  :deep(p) {
    margin: 4px 0;
  }

  :deep(code) {
    font-size: var(--font-size-sm);
  }
}

.meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  user-select: text;
  padding-left: 8px;
  .model-tag {
    background: var(--bg-hover);
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
  }
}

/* ====== 操作按钮栏 ====== */
.action-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 2px;
  margin-top: 4px;
  transition: opacity 0.2s ease;
  // border-top: 1px solid #eeeeee66;
  border-top: 1px solid var(--chat-divider-color);
  // 操作栏超出容器时自动换行（修复 #202）
  overflow: hidden;

}

.action-btn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  border-radius: 6px;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all 0.15s;


  &:hover {
    background: rgba(0, 0, 0, 0.06);
    color: var(--text-primary);
  }

  &.active {
    color: var(--color-primary);
  }

  &:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  &--danger:hover {
    color: var(--color-danger);
    background: rgba(255, 77, 79, 0.08);
  }
}

.translating-btn {
  width: auto !important;
  padding: 0 8px !important;
  gap: 4px;
  cursor: not-allowed !important;
  opacity: 0.7;

  .translating-label {
    font-size: 11px;
    white-space: nowrap;
  }

  .spin-icon {
    animation: msg-spin 1s linear infinite;
  }
}

/* ── Skill 调用标签 ── */
.skill-used-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  margin-bottom: 8px;
  border-radius: 6px;
  font-size: 12px;
  // background: rgba(245, 158, 11, 0.08);
  // border: 1px solid rgba(245, 158, 11, 0.2);
    background: var(--bg-hover, #f3f4f6);
    border: 1px solid var(--border-color, #e5e7eb);
}
.skill-badge-icon {
  font-size: 13px;
  flex-shrink: 0;
}
.skill-badge-label {
  font-weight: 600;
  font-size: 11px;
 // color: #d97706;
 // color:var(--info-ambe-text-color);
  // background: rgba(245, 158, 11, 0.12);
  // background:rgba(255, 255, 255, 0.7);
      background: #ede9fe;
    color: #7c3aed;
  padding: 1px 5px;
  border-radius: 9px;

}
.skill-badge-name {
  font-family: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace;
  font-weight: 600;
  color: var(--text-primary);
  font-size: 12px;
}
.skill-badge-args {
  color: var(--text-secondary);
  font-size: 11px;
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── 工具调用卡片容器 ── */
.tool-calls-section {
  margin-bottom: 8px;
}

/* ── AI 思考中骨架（治白框） ── */
.thinking-skeleton {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  margin-bottom: 8px;
  border-radius: 8px;
  background: rgba(100, 116, 139, 0.06);
  border: 1px solid rgba(100, 116, 139, 0.12);
}

.thinking-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border-color);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: msg-spin 0.8s linear infinite;
  flex-shrink: 0;
}

.thinking-text {
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  letter-spacing: 0.3px;
}

@keyframes msg-spin {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

.action-divider {
  width: 1px;
  height: 14px;
  background: var(--chat-divider-color);
  margin: 0 4px;
}


/* 覆盖 el-popconfirm 弹窗按钮样式 */
:deep(.el-popconfirm) {
  .el-popconfirm__main {
    color: var(--text-primary);
  }

  .el-button.el-button--small {
    border-radius: 10px;
    /* 修改圆角 */
    font-size: var(--font-size-sm);
  }

  /* 取消按钮 */
  .el-button.el-button--small:not(.el-button--primary) {
    color: var(--text-secondary);
    background: transparent;
    border-color: var(--border-color);

    &:hover {
      color: var(--text-primary);
      background: var(--bg-hover);
    }
  }

  /* 确认（删除）按钮 */
  .el-button.el-button--small.el-button--primary {
    color: #fff;
    background: var(--color-danger);
    border-color: var(--color-danger);

    &:hover {
      background: color-mix(in srgb, var(--color-danger) 85%, #000);
      border-color: color-mix(in srgb, var(--color-danger) 85%, #000);
    }
  }
}


.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* ====== 编辑标记 & 用户操作 ====== */
.edited-mark {
  font-size: 11px;
  color: var(--text-primary);
  margin-top: 4px;
  display: block;
  text-align: right;
}

.user-actions {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
  margin-top: 6px;
}

/* ====== 翻译结果 ====== */
.translate-divider {
  display: flex;
  align-items: center;
  margin: 12px 0 8px;
  gap: 8px;
  position: relative;

  .translate-divider-line {
    flex: 1;
    height: 1px;
    background: var(--border-color);
  }

  .translate-divider-icon {
    position: absolute;
    background-color: var(--bg-primary);
    z-index: 1;
    left: 50%;
    transform: translateX(-50%);
    font-size: 14px;
    color: var(--text-primary);
    user-select: none;
  }
}

.translation-section {
  padding: 4px 0;

  .translation-label {
    font-size: 11px;
    color: var(--text-tertiary);
    margin-bottom: 4px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
}

.switch-model-wrap {
  position: relative;
  width: 28px;
  height: 28px;
}

.switch-model-hidden {
  position: absolute;
  top: 0;
  left: 0;
  width: 28px;
  height: 28px;
  opacity: 0;
  pointer-events: none;
  z-index: -1;
  overflow: hidden;

  :deep(.el-select__wrapper) {
    width: 28px !important;
    min-height: 28px !important;
    padding: 0 6px !important;
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
  }

  :deep(.el-input__wrapper) {
    width: 28px;
    min-height: 28px;
    padding: 0 6px !important;
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
  }
}
</style>

<style lang="scss">
.translate-menu {
  padding: 4px;

  .dropdown-item {
    display: block;
    width: 100%;
    padding: 7px 12px;
    border: none;
    background: transparent;

    font-size: var(--font-size-sm);
    text-align: left;
    cursor: pointer;
    border-radius: 6px;

    &:hover {
      background: var(--bg-hover);
      color: var(--color-primary);
    }
  }
}

.model-item {
  width: 100%;
  display: flex;
  align-items: center;
  max-width: 340px;
  overflow: hidden;

  .model-item-title {
    max-width: 200px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .model-tag-box {
    flex: 1;
    margin-left: 10px;
    overflow: hidden;
    white-space: nowrap;

    .scroll-box {
      height: 100%;
      display: flex;
      align-items: center;

      .el-tag+.el-tag {
        margin-left: 4px;
      }
    }
  }
}
</style>


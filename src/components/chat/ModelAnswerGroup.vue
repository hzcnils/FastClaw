<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import MessageBubble from './MessageBubble.vue'
import TraceSource from './TraceSource.vue'
import DocVersionCard from './DocVersionCard.vue'
import SvgIcon from '@/components/svgIcon/index.vue';
import type { Message, ChatAttachment } from '@/types'
import { Document as DocumentIcon, ChatLineRound } from '@element-plus/icons-vue'

interface DocCardInfo { versionIdx: number; label: string; timestamp: string }

const props = defineProps<{
  messages: Message[]       // all messages with same group_id
  allMessages: Message[]    // all messages in conversation (for context)
  messageIndex: number      // index of first message in this group
  lastAiMsgId: string | null  // 只允许最后一条 AI 消息重新生成
  showBranch?: boolean        // 是否显示分支按钮
  modelGroups?: any[]         // 模型分组（用于在气泡里按所用模型显示名称/图标）
  docEditCards?: Record<string, DocCardInfo>  // doc-edit 版本卡片映射（messageId → 卡片信息）
  docEditTitle?: string       // 文档标题（用于卡片显示）
  inConversation?: boolean
  userAvatarSrc?: string
  readingId?: string | null  // 当前正在朗读的消息 ID（null 表示无朗读）
}>()

const emit = defineEmits<{
  'switch-model': [groupId: string]
  'switch-model-change': [messageId: string, targetModelId: string]
  'regenerate': [id: string]
  'delete': [id: string]
  'edit': [id: string]
  'resend-edit': [content: string]
  'translate': [id: string, lang: string]
  'favorite': [id: string, next: boolean]
  'quote': [content: string]
  'quote-doc': [doc: ChatAttachment]
  'edit-as-doc': [content: string]
  'tts': [id: string, content: string]
  'like': [id: string, next: number]
  'dislike': [id: string, next: number]
  'trace-source': [type: string, messageId: string]
  'web-search': [messageId: string]
  'copy': [content: string]
  'quality-submit': [messageId: string, score: number, content: string]
  'branch': [id: string]
  'version-change': [versionIdx: number]
}>()

const { t } = useI18n()
const currentIndex = ref(0)
// 水平拆分时右侧面板的选中索引（对应 assistantMessages 的下标，默认 1=第二个模型）
const rightPanelIndex = ref(1)
const displayMode = ref<'tabs' | 'horizontal'>('tabs')

// 用户消息内联编辑
const editingUserMsg = ref(false)
const editContent = ref('')
const isCurrentQuestion = computed(() => assistantMessages.value.some(m => m.status === 'streaming'))

function onUserEdit(id: string) {
  if (isCurrentQuestion.value) return
  const msg = props.messages.find(m => m.id === id)
  if (!msg) return
  editContent.value = msg.content
  editingUserMsg.value = true
}

function cancelUserEdit() {
  editingUserMsg.value = false
  editContent.value = ''
}

function submitUserEdit() {
  const content = editContent.value.trim()
  if (!content) return
  editingUserMsg.value = false
  editContent.value = ''
  emit('resend-edit', content)
}

const assistantMessages = computed(() =>
  props.messages.filter(m => m.role === 'assistant')
)

const userMessage = computed(() =>
  props.messages.find(m => m.role === 'user')
)

// 解析模型 ID 为显示名称
function resolveModelName(modelId: string): string {
  if (!modelId || !props.modelGroups) return t('chat.unknownModel')
  for (const g of props.modelGroups) {
    const m = g.children?.find(x => x.id === modelId || x.modelId === modelId || x.rawModelId === modelId)
    if (m) return m.name
  }
  return modelId
}

// 用户消息关联的文档附件
const docAttachments = computed<ChatAttachment[]>(() => {
  const um = userMessage.value
  if (!um?.attachments) return []
  return um.attachments.filter(
    (a) => a.type === 'document' && a.status === 'uploaded',
  )
})

function truncateFileName(name: string, maxLen: number): string {
  if (name.length <= maxLen) return name
  return name.substring(0, maxLen - 1) + '…'
}

// #1: 切换模型回答后自动选中最新答案
watch(() => assistantMessages.value.length, (len, oldLen) => {
  if (oldLen && len > oldLen) {
    currentIndex.value = len - 1
  }
})

function isPanelVisible(idx: number): boolean {
  const len = assistantMessages.value.length
  if (len <= 1) return true
  if (displayMode.value === 'horizontal') {
    // 左侧固定第一个模型，右侧显示 rightPanelIndex 对应的模型
    return idx === 0 || idx === rightPanelIndex.value
  }
  return idx === currentIndex.value
}

function onSwitchModelChange(messageId: string, targetModelId: string) {
  emit('switch-model-change', messageId, targetModelId)
}
</script>

<template>
  <div class="model-answer-group">
    <!-- 文档附件卡片（显示在用户提问上方） -->
    <div v-if="docAttachments.length" class="doc-attachments-strip">
      <div v-for="doc in docAttachments" :key="doc.id" class="doc-card" :title="doc.name">
        <el-icon :size="16">
          <DocumentIcon />
        </el-icon>
        <span class="doc-card-name">{{ truncateFileName(doc.name, 12) }}</span>
        <button class="doc-quote-btn" title="引用此文档" @click.stop="emit('quote-doc', doc)">
          <el-icon :size="14"><ChatLineRound /></el-icon>
        </button>
      </div>
    </div>

    <!-- 用户消息（只展示一次） -->
    <div v-if="userMessage" class="user-msg-wrapper">
      <div v-if="editingUserMsg" class="inline-user-edit" @click.stop>
        <button class="inline-edit-cancel" title="取消" @click="cancelUserEdit">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </button>
        <el-input v-model="editContent" class="inline-edit-input" type="textarea"
          :autosize="{ minRows: 1, maxRows: 20 }" autofocus
          @keydown.enter.exact.prevent="submitUserEdit"
          @keydown.esc.prevent="cancelUserEdit" />
        <button class="inline-edit-submit" title="完成"
          :disabled="!editContent.trim()" @click="submitUserEdit">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2.4" fill="none">
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </button>
      </div>
      <MessageBubble v-else :message="userMessage" :disable-edit="isCurrentQuestion"
        :user-avatar-src="props.userAvatarSrc"
        @delete="(id) => emit('delete', id)"
        @edit="onUserEdit" @resend-edit="(content) => emit('resend-edit', content)" />
    </div>

    <!-- 多模型切换栏 -->
    <div v-if="assistantMessages.length > 1" class="model-tabs-bar">
      <div class="tabs-container">
        <!-- tabs 模式：所有模型平铺 -->
        <div class="tabs-scroll" v-show="displayMode === 'tabs'">
          <button v-for="(msg, idx) in assistantMessages" :key="msg.id"
            :class="['tab-btn', { active: currentIndex === idx }]" @click="currentIndex = idx">
            <span class="tab-model-name">{{ resolveModelName(msg.model_id) || t('chat.unknownModel') }}</span>
          </button>
        </div>
      </div>
      <div class="mode-toggle">
        <button :class="['mode-btn', { active: displayMode === 'tabs' }]" :title="t('chat.tabSwitch')"
          @click="displayMode = 'tabs'">
          <svgIcon iconName="tabs" />
        </button>
        <button :class="['mode-btn', { active: displayMode === 'horizontal' }]" :title="t('chat.horizontalSplit')"
          @click="displayMode = 'horizontal'">
          <svgIcon iconName="horizontal" />
        </button>
      </div>
    </div>

    <!-- AI 回答展示 -->
    <div class="model-answers" :class="[displayMode, { 'single': assistantMessages.length === 1 }]">
      <template v-for="(msg, idx) in assistantMessages" :key="msg.id">
        <div v-show="isPanelVisible(idx)" class="answer-panel">
          <!-- 水平拆分：左侧只显示模型名称，右侧显示模型名称+切换 tabs -->
          <div v-if="displayMode === 'horizontal'" class="horizontal-header">
            <!-- 右侧面板：模型名称 + 本地切换 tabs -->
            <div v-if="idx !== 0" class="right-panel-tabs">
              <button v-for="(m, i) in assistantMessages.slice(1)" :key="m.id"
                :class="['tab-btn', { active: rightPanelIndex === (i + 1) }]" @click="rightPanelIndex = (i + 1)">
                {{ m.model_id || t('chat.unknownModel') }}
              </button>
            </div>
            <!-- 左侧面板只显示模型名称 -->
            <span v-if="idx === 0" class="horizontal-model-name">{{ resolveModelName(msg.model_id) || t('chat.unknownModel') }}</span>
          </div>
          <MessageBubble :message="msg" :show-switch-model="true" :show-regenerate="msg.id === props.lastAiMsgId"
            :user-avatar-src="props.userAvatarSrc" :reading-id="props.readingId"
            :show-branch="props.showBranch"
            :model-groups="props.modelGroups" :hide-doc-edit="!!props.docEditCards?.[msg.id]"
            @regenerate="(id) => emit('regenerate', id)"
            @delete="(id) => emit('delete', id)" @edit="(id) => emit('edit', id)"
            @translate="(id, lang) => emit('translate', id, lang)" @favorite="(id, next) => emit('favorite', id, next)"
            @quote="(content) => emit('quote', content)" @edit-as-doc="(content) => emit('edit-as-doc', content)"
            @tts="(id, content) => emit('tts', id, content)" @like="(id, next) => emit('like', id, next)"
            @dislike="(id, next) => emit('dislike', id, next)" @trace-source="(type, id) => emit('trace-source', type, id)"
            @web-search="(id) => emit('web-search', id)" @branch="(id) => emit('branch', id)"
            @switch-model-change="onSwitchModelChange">
            <!-- doc-edit 版本卡片（在操作栏之前），始终提供 slot 保证位置稳定 -->
            <template #below-content>
              <DocVersionCard
                v-if="props.docEditCards?.[msg.id]"
                :title="props.docEditTitle || t('chat.aiAnswer')"
                :version-label="props.docEditCards[msg.id].label"
                :timestamp="props.docEditCards[msg.id].timestamp"
                @click="emit('version-change', props.docEditCards[msg.id].versionIdx)"
              />
            </template>
          </MessageBubble>
          <TraceSource v-if="msg.role === 'assistant' && msg.trace_source" :message="msg"
            @quality-submit="(messageId, score, content) => emit('quality-submit', messageId, score, content)" />
        </div>
      </template>
    </div>
  </div>
</template>

<style lang="scss" scoped>
/* ─── 文档附件卡片 ─── */
.doc-attachments-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 6px;
  justify-content: flex-end;
  padding-right: 44px;
}

.doc-card {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 8px 4px 10px;
  border-radius: 8px;
  background: var(--bg-secondary, var(--bg-hover));
  border: 1px solid transparent;
  cursor: default;
  max-width: 200px;
  transition: border-color 0.2s, box-shadow 0.2s;

  &:hover {
    border-color: var(--color-primary, #1677ff);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .el-icon {
    flex-shrink: 0;
    color: var(--text-secondary);
  }

  .doc-quote-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    margin-left: auto;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    border-radius: 4px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s, background 0.15s;

    &:hover {
      background: var(--bg-hover);
      color: var(--color-primary, #1677ff);
    }
  }

  &:hover .doc-quote-btn {
    opacity: 1;
  }
}

.doc-card-name {
  font-size: 12px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 120px;
}

.user-msg-wrapper {
  margin-bottom: 4px;
}

.model-tabs-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  padding: 0 4px;
}

.tabs-container {
  flex: 1;
  overflow-x: auto;
  scrollbar-width: none;

  &::-webkit-scrollbar {
    display: none;
  }
}

.tabs-scroll {
  display: flex;
  gap: 4px;
}

.tab-btn {
  padding: 4px 14px;
  border: 1px solid var(--border-color);
  border-radius: 16px;
  background: var(--bg-primary);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;

  &:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  &.active {
    background: color-mix(in srgb, var(--color-primary) 12%, var(--bg-primary));
    color: var(--color-primary);
    border-color: color-mix(in srgb, var(--color-primary) 30%, var(--border-color));
    font-weight: 500;
  }
}

.tab-model-name {
  font-size: 12px;
}

.mode-toggle {
  display: flex;
  gap: 2px;
  margin-left: 8px;
  flex-shrink: 0;
}

.mode-btn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  &.active {
    background: var(--bg-active);
    color: var(--color-primary);
  }
}

.model-answers {
  &.horizontal {
    display: flex;
    gap: 12px;

    .answer-panel {
      flex: 1;
      min-width: 0;

      &:first-child {
        border-right: 1px solid var(--border-color);
        padding-right: 12px;
      }

      &:last-child {
        padding-left: 12px;
      }
    }

    .horizontal-header {
      display: flex;
      align-items: center;
      margin-bottom: 10px;
      padding-bottom: 8px;
      border-bottom: 1px solid var(--border-color);
    }

    .horizontal-model-name {
      font-size: 12px;
      font-weight: 600;
      color: var(--color-primary);
      background: color-mix(in srgb, var(--color-primary) 10%, transparent);
      padding: 3px 10px;
      border-radius: 6px;
    }

    .right-panel-tabs {
      display: flex;
      gap: 4px;
      overflow-x: auto;
      flex: 1;
      scrollbar-width: none;

      &::-webkit-scrollbar {
        display: none;
      }

      .tab-btn {
        padding: 3px 12px;
        border: 1px solid var(--border-color);
        border-radius: 14px;
        background: var(--bg-primary);
        color: var(--text-secondary);
        font-size: 12px;
        cursor: pointer;
        transition: all 0.15s;
        white-space: nowrap;
        flex-shrink: 0;

        &:hover {
          background: var(--bg-hover);
          color: var(--text-primary);
        }

        &.active {
          background: color-mix(in srgb, var(--color-primary) 12%, var(--bg-primary));
          color: var(--color-primary);
          border-color: color-mix(in srgb, var(--color-primary) 30%, var(--border-color));
          font-weight: 500;
        }
      }
    }
  }

  &.tabs {
    border: none;
    background-color: initial;

    .answer-panel {
      min-width: 0;
    }
  }

  &.single {
    .single-model-label {
      margin-bottom: 4px;
      padding-left: 48px;

      .model-badge {
        font-size: 11px;
        color: var(--text-secondary);
        background: var(--bg-hover);
        padding: 2px 8px;
        border-radius: 4px;
      }
    }
  }
}

.inline-user-edit {
  display: flex;
  align-items: center;
  gap: 10px;
  width: min(760px, 100%);
  margin-left: auto;
  padding: 12px 16px;
}

.inline-edit-cancel,
.inline-edit-submit {
  width: 30px;
  height: 30px;
  border: none;
  background: transparent;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
  color: var(--text-secondary);
  border-radius: 50%;
  transition: color 0.15s, background 0.15s;

  &:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  &:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }
}

.inline-edit-submit {
  background: var(--color-primary);
  color: #fff;

  &:hover:not(:disabled) {
    background: var(--color-primary);
    color: #fff;
    opacity: 0.9;
  }
}

.inline-edit-input {
  flex: 1;
  min-width: 0;

  :deep(.el-textarea__inner) {
    resize: none;
    line-height: 1.6;
    font-size: var(--font-size-base);
    color: var(--text-primary);
    border-radius: 10px;
    border: 2px solid var(--color-primary);
    box-shadow: none !important;
    background: var(--bg-primary);
  }
}
</style>

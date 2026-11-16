<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'
import { Edit } from '@element-plus/icons-vue'
import SvgIcon from '@/components/svgIcon/index.vue'

const { t } = useI18n()

const props = defineProps<{
  model: {
    id: string
    model_id?: string
    display_name?: string
    model_name?: string
    model_type?: string
    model_label?: string
    is_enabled?: boolean
    context_window?: number | string | null
    context_length?: number | string
  }
  providerCode: string
  manageMode?: boolean
  isSelected?: boolean
  rating?: { tier: string; likes: number; dislikes: number; score: number } | null
}>()

const emit = defineEmits<{
  (e: 'edit', model: any): void
  (e: 'delete', model: any): void
  (e: 'toggle', model: any): void
  (e: 'select', model: any): void
}>()

// 模型名称关键词 → 图标名称映射（顺序敏感：更具体的关键词优先）
const MODEL_ICON_MAP: Array<[string, string]> = [
  ['ollama', 'OLLAMA'],
  ['llamacpp', 'LLAMACPP'],
  ['llama', 'LLAMACPP'],
  ['claude', 'CLAUDE'],
  ['openai', 'OPENAI'],
  ['gpt', 'OPENAI'],
  ['gemini', 'GEMINI'],
  ['deepseek', 'DEEPSEEK'],
  ['qwen', 'Q_WEN'],
  ['doubao', 'DOUYIN'],
  ['bytedance', 'DOUYIN'],
  ['chatglm', 'ZHIPU'],
  ['zhipu', 'ZHIPU'],
  ['glm', 'ZHIPU'],
  ['hunyuan', 'HUNYUAN'],
  ['ernie', 'BAIDU'],
  ['wenxin', 'BAIDU'],
  ['baidu', 'BAIDU'],
  ['spark', 'SPARKDESK'],
]

function matchIcon(text?: string): string | null {
  if (!text) return null
  const lower = text.toLowerCase()
  for (const [keyword, icon] of MODEL_ICON_MAP) {
    if (lower.includes(keyword)) return icon
  }
  return null
}

const modelIconName = computed(
  () => matchIcon(props.model.display_name) ?? matchIcon(props.providerCode) ?? props.providerCode,
)

function handleClick() {
  if (props.manageMode) {
    emit('select', props.model)
  }
}

// 主显示名称：优先 display_name，其次 model_name，最后 model_id
const mainName = () =>
  props.model.display_name || props.model.model_name || props.model.model_id || props.model.id

// 副标题 model_id：当 model_id 与主名称不同时展示
const subModelId = () => {
  const id = props.model.model_id
  return id ? id : null
}

const labelChips = () => {
  if (!props.model.model_label) return []
  return props.model.model_label.split(/[,，]/).map(s => s.trim()).filter(Boolean)
}

/**
 * 能力标签 → 配色方案。
 * 语义化关键词优先匹配（对话=蓝 / 视觉=紫 / 推理=橙 / 工具=绿 …），
 * 未命中的标签（如供应商名）按内容哈希到一个稳定颜色，保证同名标签颜色一致。
 */
const CHIP_PALETTES = ['blue', 'purple', 'orange', 'green', 'teal', 'rose', 'indigo'] as const

const CHIP_KEYWORDS: Array<[RegExp, (typeof CHIP_PALETTES)[number]]> = [
  [/对话|chat|dialog/i, 'blue'],
  [/视觉|多模态|图像|vision|image|multimodal|vl/i, 'purple'],
  [/推理|思考|reason|think/i, 'orange'],
  [/工具|函数|tool|function/i, 'green'],
  [/嵌入|embed/i, 'teal'],
  [/重排|rerank/i, 'rose'],
  [/语音|voice|audio|tts|stt|asr/i, 'indigo'],
]

function chipClass(text: string): string {
  for (const [re, palette] of CHIP_KEYWORDS) {
    if (re.test(text)) return `chip-${palette}`
  }
  // 稳定哈希回退，确保同一标签每次渲染颜色一致
  let hash = 0
  for (let i = 0; i < text.length; i++) hash = (hash * 31 + text.charCodeAt(i)) >>> 0
  return `chip-${CHIP_PALETTES[hash % CHIP_PALETTES.length]}`
}

const isEnabled = computed(() => props.model.is_enabled !== false)

const TIER_CLASS: Record<string, string> = {
  excellent: 'mc-rate-excellent',
  good:      'mc-rate-good',
  neutral:   'mc-rate-neutral',
  bad:       'mc-rate-bad',
  new:       'mc-rate-new',
  none:      'mc-rate-none',
}
const ratingChip = computed(() => {
  const tier = props.rating?.tier ?? 'none'
  return {
    text: t(`modelManage.rate_${tier}`),
    cls: TIER_CLASS[tier] || TIER_CLASS.none,
  }
})
</script>

<template>
  <div
    class="model-card"
    :class="{
      'manage-mode': manageMode,
      'is-selected': manageMode && isSelected,
      'is-disabled': !manageMode && !isEnabled,
    }"
    @click="handleClick"
  >
    <!-- 上部分：Logo + 模型信息 + 操作按钮 -->
    <div class="mc-top">
      <!-- 左侧：模型 Logo -->
      <div class="mc-logo">
        <SvgIcon :iconName="modelIconName" style="font-size: 30px" />
      </div>

      <!-- 中间：显示名称 + model_id -->
      <div class="mc-info">
        <div class="mc-title" :title="mainName()">
          {{ mainName() }}
        </div>
        <div v-if="subModelId()" class="mc-model-id" :title="subModelId() || ''">
          {{ subModelId() }}
        </div>
      </div>

      <!-- 右侧：管理复选框 / 操作按钮 -->
      <template v-if="manageMode">
        <div class="mc-checkbox" :class="{ checked: isSelected }">
          <Check v-if="isSelected" class="check-icon" />
        </div>
      </template>
      <template v-else>
        <div class="mc-actions">
          <!-- 编辑按钮 -->
          <button class="mc-btn" :title="$t('modelManage.editTooltip')" @click.stop="emit('edit', model)">
            <el-icon :size="16"><Edit /></el-icon>
          </button>
          <!-- 删除按钮 -->
          <button class="mc-btn mc-btn-delete" :title="$t('modelManage.deleteTooltip')" @click.stop="emit('delete', model)">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
          </button>
        </div>
      </template>
    </div>

    <!-- 下部分：能力标签 + 启用开关 -->
    <div class="mc-bottom">
      <div class="mc-chips">
        <template v-if="labelChips().length">
          <span
            v-for="chip in labelChips()"
            :key="chip"
            class="mc-chip"
            :class="chipClass(chip)"
          >
            <el-icon :size="14"><SvgIcon :iconName="chip" /></el-icon>
            {{ chip }}
          </span>
        </template>
        <span v-else class="mc-label-empty">{{ $t('modelManage.noTags') }}</span>
      </div>

      <!-- 启用/禁用开关（管理模式下隐藏，避免与多选冲突） -->
      <button
        v-if="!manageMode"
        class="mc-toggle"
        :class="{ enabled: isEnabled }"
        @click.stop="emit('toggle', model)"
        :title="isEnabled ? $t('modelManage.enabled') : $t('modelManage.disabled')"
      >
        <span class="mc-toggle-thumb" />
      </button>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.model-card {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 16px 18px 14px;
  border-radius: 16px;
 // background: linear-gradient(125deg, #f4f7ff 0%, #fafbff 40%, #ffffff 74%);
  border: 1px solid rgba(99, 102, 241, 0.10);
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.04);
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
  cursor: default;
  overflow: hidden;

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 10px 24px rgba(16, 24, 40, 0.10);
    border-color: rgba(99, 102, 241, 0.28);
  }

  &.is-disabled {
    opacity: 0.6;
    filter: grayscale(0.25);
  }

  &.manage-mode {
    cursor: pointer;

    &.is-selected {
      border-color: var(--color-primary) !important;
      box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-primary) 28%, transparent);
    }
  }
}

/* ========== 上部分 ========== */
.mc-top {
  display: flex;
  align-items: center;
  gap: 12px;
}

.mc-logo {
  flex-shrink: 0;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
}

.mc-info {
  flex: 1;
  min-width: 0;
}

.mc-title {
  font-weight: 700;
  font-size: 15px;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.35;
}

.mc-model-id {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 3px;
  line-height: 1.3;
}

/* ========== 复选框 ========== */
.mc-checkbox {
  width: 20px;
  height: 20px;
  border: 1px solid var(--text-secondary);
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all 0.15s;

  &.checked {
    border-color: var(--color-primary);
    background: var(--color-primary);
  }

  .check-icon {
    width: 14px;
    height: 14px;
    color: #fff;
  }
}

/* ========== 操作按钮区 ========== */
.mc-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
  align-self: flex-start;
  margin-top: -2px;
}

.mc-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: #9aa3b2;
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: rgba(99, 102, 241, 0.10);
    color: var(--color-primary);
  }

  &.mc-btn-delete:hover {
    background: rgba(244, 63, 94, 0.10);
    color: #f43f5e;
  }
}

/* ========== 下部分：标签 + 开关 ========== */
.mc-bottom {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
}

.mc-chips {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.mc-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 22px;
  padding: 0 9px;
  font-size: 12px;
  font-weight: 500;
  line-height: 1;
  border-radius: 7px;
  border: 1px solid transparent;
  white-space: nowrap;
}

/* 语义配色 —— 浅底 + 同色文字 + 同色描边 */
.chip-blue   { color: #2563eb; background: #eff5ff; border-color: #c7dbff; }
.chip-purple { color: #7c3aed; background: #f5f1ff; border-color: #ddd0fb; }
.chip-orange { color: #ea7517; background: #fff5eb; border-color: #ffdcb8; }
.chip-green  { color: #15a34a; background: #ecfdf3; border-color: #b9eccb; }
.chip-teal   { color: #0d9488; background: #effcfa; border-color: #b3ece6; }
.chip-rose   { color: #e11d63; background: #fff1f6; border-color: #fbcfdf; }
.chip-indigo { color: #4f46e5; background: #eef0ff; border-color: #cdd2fb; }

.mc-label-empty {
  font-size: 12px;
  color: var(--text-secondary);
  opacity: 0.5;
}


/* ========== Toggle 开关 ========== */
.mc-toggle {
  position: relative;
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  width: 34px;
  height: 19px;
  border: none;
  border-radius: 19px;
  background: #d1d5db;
  cursor: pointer;
  transition: background 0.25s;
  padding: 0;

  &.enabled {
    background: #22c55e;
  }

  .mc-toggle-thumb {
    position: absolute;
    left: 2px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.25s;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  }

  &.enabled .mc-toggle-thumb {
    transform: translateX(15px);
  }
}

/* ========== 情景主题（除雾灰调）适配 ========== */
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-card {
  background:   rgba(255, 255, 255, 0.15) !important;
  border-color: rgba(99, 102, 241, 0.10) !important;
}
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-card.manage-mode.is-selected {
  border-color: var(--color-primary) !important;
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-primary) 28%, transparent);
}

/* 情景主题（除雾灰调）下编辑按钮图标改为白色 */
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mc-btn {
  color: #fff;
}
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mc-btn:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.15);
}
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mc-btn.mc-btn-delete:hover {
  background: rgba(244, 63, 94, 0.25);
  color: #fff;
}

/* ========== 暗色主题适配 ========== */
:global(html.dark) .model-card {
  background: linear-gradient(125deg, rgba(124, 140, 255, 0.07) 0%, rgba(255, 255, 255, 0.03) 44%, rgba(255, 255, 255, 0.02) 100%);
  border-color: rgba(255, 255, 255, 0.10);
  box-shadow: none;
}

:global(html.dark) .model-card:hover {
  border-color: rgba(99, 102, 241, 0.45);
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
}

:global(html.dark) .model-card.manage-mode.is-selected,
:global(html.dark) .model-card.manage-mode.is-selected:hover {
  border-color: var(--color-primary) !important;
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-primary) 28%, transparent);
}

/* 暗色下标签略微提亮文字、降低底色不透明度 */
:global(html.dark) .mc-chip {
  background: transparent;
}
:global(html.dark) .chip-blue   { color: #93b4ff; border-color: rgba(96, 145, 255, 0.45); background: rgba(96, 145, 255, 0.12); }
:global(html.dark) .chip-purple { color: #c4abff; border-color: rgba(160, 120, 255, 0.45); background: rgba(160, 120, 255, 0.12); }
:global(html.dark) .chip-orange { color: #ffba7a; border-color: rgba(255, 150, 70, 0.45); background: rgba(255, 150, 70, 0.12); }
:global(html.dark) .chip-green  { color: #74e3a3; border-color: rgba(60, 200, 120, 0.45); background: rgba(60, 200, 120, 0.12); }
:global(html.dark) .chip-teal   { color: #6fe0d4; border-color: rgba(40, 200, 185, 0.45); background: rgba(40, 200, 185, 0.12); }
:global(html.dark) .chip-rose   { color: #ff97bd; border-color: rgba(240, 90, 140, 0.45); background: rgba(240, 90, 140, 0.12); }
:global(html.dark) .chip-indigo { color: #a8b0ff; border-color: rgba(120, 130, 255, 0.45); background: rgba(120, 130, 255, 0.12); }
</style>

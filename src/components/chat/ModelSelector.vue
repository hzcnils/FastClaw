<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed, nextTick } from 'vue'
import selectModel from './selectModel.vue'
import { parseTags, toTagObjects } from '@/utils/modelUtils'
import { isHiddenReleaseProvider } from '@/utils/provider'
import { ElSelect } from 'element-plus'
import { View, Hide } from '@element-plus/icons-vue'
import SvgIcon from '@/components/svgIcon/index.vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
// const MODEL_STORAGE_KEY = 'fastclaw-selected-model'
const model = defineModel<string | undefined>()

const PRESET_CHIPS = ['视觉', '联网', '推理', '工具', '重排', '嵌入']

const props = withDefaults(defineProps<{
  hideAdvanced?: boolean
  popperClass?: string
  iconTags?: boolean
  modelGroups?: ModelGroup[]
  showIncompleteFilter?: boolean
  isVendorIncomplete?: (providerCode: string) => boolean
  /** 面板固定在触发器下方，不向上翻转盖住选中值 */
  lockPlacementBottom?: boolean
  /** 可选：覆盖默认 placement（不影响 lockPlacementBottom） */
  placement?: string
  /** 可选：覆盖 fallback placements，便于上下自适应 */
  fallbackPlacements?: string[]
  /** 可选：自定义 popper-options（不影响 lockPlacementBottom） */
  popperOptions?: Record<string, unknown>
}>(), {
  hideAdvanced: false,
  popperClass: 'model-selector-popper',
  iconTags: false,
  showIncompleteFilter: false,
  lockPlacementBottom: false,
})

// ─── 模型数据结构（与 ChatView.vue 保持一致） ───
interface ModelGroup {
  id: string
  label: string
  providerCode?: string
  sourceCategory?: string
  children: Array<{
    id: string
    modelId: string
    rawModelId: string
    name: string
    modelLabel: string
    tags: Array<{ value: string; label: string }>
  }>
}

/**
 * 根据供应商代码/分类获取图标名称，与 ChatView.vue 选择模型下拉框保持一致。
 * 汇智网/私域网/本电脑使用固定图标；
 * 其他供应商使用 source_category 对应的图标（local→本地, online→互联网, huizhi→汇智网）。
 */
function getProviderIcon(providerCode?: string, sourceCategory?: string): string {
  const knownIcons: Record<string, string> = {
    HUIZHI: "CI",  // 汇智网
    PRIVATE: "lan",      // 私域网
    LOCAL: "local",      // 本电脑
  }
  if (providerCode && knownIcons[providerCode]) return knownIcons[providerCode]

  const categoryIcons: Record<string, string> = {
    local: "local",
    online: "internet",
    huizhi: "CI",
  }
  return (sourceCategory && categoryIcons[sourceCategory]) || "internet"
}

// ─── 状态 ───
const modelTree = ref<ModelGroup[]>([])
const loading = ref(false)
const searchModelStr = ref('')
const selectModelRef = ref<InstanceType<typeof selectModel> | null>(null)
const selectModelFlag = ref(false)
const modelSelector = ref<InstanceType<typeof ElSelect> | null>(null)
const hideIncompleteModels = ref(false)
/** 下拉打开时才显示未配置提示，关闭/点选时锁定避免左上角闪烁 */
const selectVisible = ref(false)
const vendorTipLocked = ref(false)

function onSelectVisibleChange(visible: boolean) {
  selectVisible.value = visible
  vendorTipLocked.value = !visible
  emit('visible-change', visible)
}

function lockVendorTip() {
  vendorTipLocked.value = true
}

function isGroupIncomplete(g: ModelGroup): boolean {
  return !!(props.isVendorIncomplete && g.providerCode && props.isVendorIncomplete(g.providerCode))
}

// ─── 过滤后的列表 ───
const showModelList = computed<ModelGroup[]>(() => {
  let source = props.modelGroups || modelTree.value
  if (props.showIncompleteFilter && hideIncompleteModels.value && props.isVendorIncomplete) {
    source = source.filter((g) => !props.isVendorIncomplete!(g.providerCode ?? ''))
  }
  const kw = searchModelStr.value.trim().toLowerCase()
  if (!kw) return source
  return source
    .map((g) => ({
      id: g.id,
      label: g.label,
      providerCode: g.providerCode,
      sourceCategory: g.sourceCategory,
      children: g.children.filter(
        (m) =>
          m.name.toLowerCase().includes(kw) ||
          m.modelLabel.toLowerCase().includes(kw),
      ),
    }))
    .filter((g) => g.children.length > 0)
})

// 当前选中模型所属分组的图标
const selectedModelIcon = computed(() => {
  if (!model.value) return "internet"
  const source = props.modelGroups || modelTree.value
  for (const g of source) {
    if (g.children.some((m) => m.id === String(model.value))) {
      return getProviderIcon(g.providerCode, g.sourceCategory)
    }
  }
  return "internet"
})

// ─── 数据加载 ───
type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
let _invoke: InvokeFn | null = null
async function getInvoke(): Promise<InvokeFn | null> {
  if (_invoke) return _invoke
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    _invoke = invoke as InvokeFn
    return _invoke
  } catch { return null }
}

async function loadModels() {
  loading.value = true
  const invoke = await getInvoke()
  if (!invoke) {
    loading.value = false
    return
  }

  try {
    const tree: any[] = (await invoke("list_models_tree")) || []
    modelTree.value = tree.filter((p: any) => !isHiddenReleaseProvider(
      p.providerCode || p.provider_code || (p.platformId === 'local' ? 'LOCAL' : ''),
      p.platformId,
    )).map((p: any) => ({
      id: p.platformId,
      label: p.platformName || p.platform_name || p.name || t("chat.defaultPlatform"),
      providerCode: p.providerCode || (p.platformId === "local" ? "LOCAL" : ""),
      sourceCategory: p.sourceCategory || "online",
      children: (p.modelList || p.models || p.children || []).map((m: any) => {
        const modelLabel = m.modelLabel || m.model_label || ""
        return {
          id: String(m.modelId || m.model_id || m.id),
          modelId: String(m.modelId || m.model_id || m.id),
          rawModelId: String(m.rawModelId || m.raw_model_id || ""),
          name: m.modelName || m.model_name || m.display_name || m.name || "",
          modelLabel,
          tags: toTagObjects(parseTags(modelLabel)),
        }
      }),
    }))
    loading.value = false
    return
  } catch {
    /* fallback to flat list */
  }

  try {
    const list = await invoke<any[]>("list_models")
    const enabled = list.filter((m: any) => m.is_enabled !== false && m.provider_code && !isHiddenReleaseProvider(m.provider_code))
    const groupMap = new Map<string, any[]>()
    for (const m of enabled) {
      const key = m.provider_name || t("chat.defaultPlatform")
      if (!groupMap.has(key)) groupMap.set(key, [])
      groupMap.get(key)!.push(m)
    }
    modelTree.value = Array.from(groupMap.entries()).map(([name, models], i) => ({
      id: String(i),
      label: name,
      providerCode: "",
      sourceCategory: "online",
      children: models.map((m: any) => {
        const modelLabel = m.model_label || ""
        return {
          id: String(m.id || m.model_id || ""),
          modelId: String(m.id || m.model_id || ""),
          rawModelId: String(m.raw_model_id || ""),
          name: m.display_name || m.model_id || "",
          modelLabel,
          tags: toTagObjects(parseTags(modelLabel)),
        }
      }),
    }))
  } catch (err) {
    console.warn("Failed to load models for selector", err)
  }
  loading.value = false
}

function groupLabel(g: ModelGroup): string {
  return g.label
}

const bottomLockPopperOptions = {
  modifiers: [
    { name: 'flip', enabled: false },
    { name: 'offset', options: { offset: [0, 8] } },
  ],
}

const lastUsableModel = ref<string | undefined>()

const emit = defineEmits<{
  (e: 'change', value: string | undefined): void
  (e: 'visible-change', value: boolean): void
}>()

async function onModelSelectorChange(id: string | undefined) {
  emit('change', id)
  if (!id) return
  const group = modelTree.value.find((g) => g.children.some((m) => m.id === id))
  lastUsableModel.value = id
}

function focus() {
  modelSelector.value?.focus()
}

function blur() {
  modelSelector.value?.blur()
}

defineExpose({ focus, blur, modelSelector })

const advancedSearch = () => {
  selectModelFlag.value = true
  nextTick(() => {
    selectModelRef.value?.open()
    modelSelector.value?.blur()
  })
}

const modelChange = (e: string) => {
  if (e) {
    model.value = e
    selectModelFlag.value = false
  }
}

function onModelCatalogUpdated() {
  // 外层传入 modelGroups 时由父级/catalog 刷新；自管列表时跟模型管理同步
  if (!props.modelGroups) loadModels()
}

onMounted(async () => {
  // ChatView supplies the shared catalog. Standalone selectors keep their
  // fallback loader, but only execute it once when actually mounted.
  document.addEventListener('fastclaw:model-updated', onModelCatalogUpdated)
  if (!props.modelGroups) await loadModels()
})

onUnmounted(() => {
  document.removeEventListener('fastclaw:model-updated', onModelCatalogUpdated)
})

// watch(model, (val: string | undefined) => {
//   if (val) {
//     localStorage.setItem(MODEL_STORAGE_KEY, String(val))
//   }
// })
</script>

<template>
  <el-select v-model="model" ref="modelSelector" class="model-selector"
      :placeholder="showModelList.length === 0 ? $t('chat.noModel') : $t('chat.selectModel')"
      :loading="loading" :popper-class="props.popperClass" :teleported="true"
      :placement="props.lockPlacementBottom ? 'bottom-start' : (props.placement || 'bottom')"
      :fallback-placements="props.lockPlacementBottom
        ? ['bottom-start', 'bottom', 'bottom-end']
        : props.fallbackPlacements"
      :popper-options="props.lockPlacementBottom ? bottomLockPopperOptions : props.popperOptions"
      :no-data-text="$t('chat.selectModel')"
      @change="onModelSelectorChange"
      @visible-change="onSelectVisibleChange">
      <template #prefix>
        <span class="model-selector-prefix">
          <el-icon :size="16">
            <svg-icon :iconName="selectedModelIcon" />
          </el-icon>
        </span>
      </template>
      <el-option-group v-for="g in showModelList" :key="g.id" :label="groupLabel(g)">
        <el-option v-for="m in g.children" :key="m.id" :label="m.name" :value="m.id">
            <el-tooltip
              :disabled="!isGroupIncomplete(g) || !selectVisible || vendorTipLocked"
              :content="t('chat.vendorIncompleteHint')"
              placement="top"
              :show-after="500"
              effect="dark"
              :hide-after="0"
            >
              <div
                class="model-item"
                :class="{ 'vendor-incomplete': isGroupIncomplete(g) }"
                @mousedown.capture="lockVendorTip"
              >
                <el-icon :size="18" style="margin-right: 4px" :title="groupLabel(g)">
                  <svg-icon :iconName="getProviderIcon(g.providerCode, g.sourceCategory)" />
                </el-icon>
                <div class="model-item-title" :title="m.name">
                  {{ m.name }}
                </div>
                <div class="model-tag-box" v-if="m.tags.length">
                  <div class="scroll-box" :title="m.modelLabel">
                    <template v-for="(tag, tagInd) in m.tags" :key="tagInd">
                      <!-- 与智能对话 ChatView 一致：不加 size="small"，标签样式走 main-chat-select-popper -->
                      <el-tag type="primary" effect="light" v-if="props.iconTags && PRESET_CHIPS.includes(tag.label || tag.value)">
                        <el-icon :size="14" :title="tag.label || tag.value">
                          <svg-icon :iconName="tag.label || tag.value" />
                        </el-icon>
                      </el-tag>
                      <el-tag type="primary" effect="light" v-else>
                        {{ tag.label || (typeof tag === 'string' ? tag : '') }}
                      </el-tag>
                    </template>
                  </div>
                </div>
              </div>
            </el-tooltip>
        </el-option>
      </el-option-group>
      <template #header>
        <el-input v-model="searchModelStr" :placeholder="$t('chat.searchModel')" clearable>
          <template v-if="props.showIncompleteFilter" #prepend>
            <el-tooltip
              :content="hideIncompleteModels ? $t('chat.showAllModels') : $t('chat.hideIncompleteModels')"
              placement="top"
              popper-class="model-selector-tooltip"
              :teleported="true">
              <el-button :type="hideIncompleteModels ? 'warning' : 'default'" size="small" @click="hideIncompleteModels = !hideIncompleteModels">
                <el-icon size="16">
                  <Hide v-if="hideIncompleteModels" />
                  <View v-else />
                </el-icon>
              </el-button>
            </el-tooltip>
          </template>
          <template v-if="!props.hideAdvanced" #append>
            <el-button @click="advancedSearch">{{ $t('chat.advanced') }}</el-button>
          </template>
        </el-input>
      </template>
    </el-select>
  <select-model v-if="selectModelFlag" ref="selectModelRef" @selectModelData="modelChange" />
</template>

<style lang="scss">
/* 下拉面板样式（非 scoped，因为 el-select 的 popper teleported 到 body） */
.model-selector-popper {
  --el-font-size-base: var(--font-size-md);
  z-index: 99999 !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 8px !important;
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;

   color:var(--text-color-light-black)!important;
    color: var(--text-primary) !important;
    font-size: var(--font-size-md) !important;
    transition: background 0.15s;
    background-color: var(--theme-dailog-color) !important;
    // 字体渲染优化（修复 #203 模型切换字体模糊）
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: optimizeLegibility;

    // 下拉选项悬停/选中：与智能对话输入框 main-chat-select-popper 一致（浅色条，非主题色实心）
    .el-select-dropdown__item {
      display: flex;
      align-items: center;
      color: var(--text-primary) !important;
      background: transparent !important;
      border-radius: 4px;
      padding: 8px 12px;
      transition: background 0.15s, color 0.15s;

      &:hover, &.is-hovering {
        background: var(--bg-hover, rgba(255, 255, 255, 0.08)) !important;
        color: var(--text-primary) !important;
      }

      &.is-selected, &.selected {
        color: var(--text-primary) !important;
        background: color-mix(in srgb, var(--color-primary) 12%, transparent) !important;
        font-weight: 600;
      }
    }

    .el-select-group__title {
      font-size: var(--font-size-sm);
      color: var(--text-secondary) !important;
      padding: 8px 12px 4px;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .el-select-dropdown__empty {
      color: var(--text-secondary) !important;
      font-size: var(--font-size-md);
      padding: 12px;
    }

    /* 搜索输入框样式 */
    .el-select-dropdown__search {
      .el-input__wrapper {
        background: var(--bg-submenu, var(--bg-primary)) !important;
        border-color: var(--border-color) !important;
        box-shadow: none !important;

        .el-input__inner {
          color: var(--text-primary) !important;
          background: transparent !important;

          &::placeholder {
            color: var(--text-secondary) !important;
          }
        }
      }

      .el-button {
        font-size: var(--font-size-md) !important;
      }
    }
  }
.model-selector-popper .el-tag,
.model-selector-popper .el-tag--primary,
.model-selector-popper .el-tag .el-tag__content {
  background: color-mix(in srgb, var(--color-primary) 5%, #ffffff) !important;
  border: 1px solid color-mix(in srgb, var(--color-primary) 8%, transparent) !important;
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
}

.model-selector-popper .el-tag .el-icon,
.model-selector-popper .el-tag svg,
.model-selector-popper .el-tag svg * {
  /* 与智能对话输入框 main-chat-select-popper 能力标签同色 */
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
  fill: currentColor !important;
}
.model-selector-popper .el-tag .el-icon svg,
.model-selector-popper .el-tag svg {
  background: transparent !important;
}

/* 悬停/选中行时保持标签样式，避免变成实心主题色块 */
.model-selector-popper .el-select-dropdown__item:hover .el-tag,
.model-selector-popper .el-select-dropdown__item.is-hovering .el-tag,
.model-selector-popper .el-select-dropdown__item.is-selected .el-tag,
.model-selector-popper .el-select-dropdown__item.selected .el-tag,
.model-selector-popper .el-select-dropdown__item:hover .el-tag .el-tag__content,
.model-selector-popper .el-select-dropdown__item.is-hovering .el-tag .el-tag__content,
.model-selector-popper .el-select-dropdown__item.is-selected .el-tag .el-tag__content,
.model-selector-popper .el-select-dropdown__item.selected .el-tag .el-tag__content {
  background: color-mix(in srgb, var(--color-primary) 5%, #ffffff) !important;
  border-color: color-mix(in srgb, var(--color-primary) 8%, transparent) !important;
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
}
.model-selector-popper .el-select-dropdown__item:hover .el-tag .el-icon,
.model-selector-popper .el-select-dropdown__item:hover .el-tag svg,
.model-selector-popper .el-select-dropdown__item:hover .el-tag svg *,
.model-selector-popper .el-select-dropdown__item.is-hovering .el-tag .el-icon,
.model-selector-popper .el-select-dropdown__item.is-hovering .el-tag svg,
.model-selector-popper .el-select-dropdown__item.is-hovering .el-tag svg *,
.model-selector-popper .el-select-dropdown__item.is-selected .el-tag .el-icon,
.model-selector-popper .el-select-dropdown__item.is-selected .el-tag svg,
.model-selector-popper .el-select-dropdown__item.is-selected .el-tag svg *,
.model-selector-popper .el-select-dropdown__item.selected .el-tag .el-icon,
.model-selector-popper .el-select-dropdown__item.selected .el-tag svg,
.model-selector-popper .el-select-dropdown__item.selected .el-tag svg * {
  color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
  fill: currentColor !important;
}

html.dark .model-selector-popper .el-tag,
html.dark .model-selector-popper .el-tag--primary,
html.dark .model-selector-popper .el-tag .el-tag__content,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-selector-popper .el-tag,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-selector-popper .el-tag--primary,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-selector-popper .el-tag .el-tag__content {
  background: color-mix(in srgb, var(--color-primary) 12%, transparent) !important;
  border: 1px solid color-mix(in srgb, var(--color-primary) 16%, transparent) !important;
  color: color-mix(in srgb, var(--color-primary) 60%, #ffffff) !important;
}
html.dark .model-selector-popper .el-tag .el-icon,
html.dark .model-selector-popper .el-tag svg,
html.dark .model-selector-popper .el-tag svg *,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-selector-popper .el-tag .el-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-selector-popper .el-tag svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .model-selector-popper .el-tag svg * {
  color: color-mix(in srgb, var(--color-primary) 60%, #ffffff) !important;
  fill: currentColor !important;
}

.model-selector-popper[data-popper-placement^='bottom'] {
  margin-top: -35px !important;
}


.model-selector-tooltip {
  z-index: 99999 !important;
}

.model-selector-popper {
  --el-tag-text-color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
  --el-tag-bg-color: color-mix(in srgb, var(--color-primary) 5%, #ffffff) !important;
  --el-tag-border-color: color-mix(in srgb, var(--color-primary) 8%, transparent) !important;

  .vendor-incomplete .model-item-title {
    opacity: 0.45;
  }
}

/* 仅改眼睛图标颜色，与下拉列表文字一致 */
.model-selector-popper .el-select-dropdown__header .el-button:not(.el-button--warning),
.model-selector-popper .el-select-dropdown__header .el-button:not(.el-button--warning) span,
.model-selector-popper .el-select-dropdown__header .el-button:not(.el-button--warning) .el-icon,
.model-selector-popper .el-select-dropdown__header .el-button:not(.el-button--warning) svg {
  color: var(--text-color-light-black, var(--text-primary)) !important;
  --el-button-text-color: var(--text-color-light-black, var(--text-primary)) !important;
  fill: currentColor !important;
}

/* 仅改搜索框占位符：比正常文字略浅 */
.model-selector-popper .el-select-dropdown__header .el-input__inner::placeholder,
.model-selector-popper .el-select-dropdown__header .el-input__inner::-webkit-input-placeholder {
  color: color-mix(in srgb, var(--text-color-light-black, var(--text-primary)) 55%, transparent) !important;
  -webkit-text-fill-color: color-mix(in srgb, var(--text-color-light-black, var(--text-primary)) 55%, transparent) !important;
  opacity: 1 !important;
}
</style>

<style lang="scss" scoped>
.model-select-wrapper {
  width: 100%;
  position: relative;
  display: inline-flex;
  align-items: center;
}

.model-select-prefix-icon {
  position: absolute;
  left: 12px;
  top: 50%;
  transform: translateY(-50%);
  z-index: 1;
  pointer-events: none;
  display: flex;
  align-items: center;
  color: var(--text-secondary);
}

.model-selector {
  width: 200px;
    
  /* trigger 视觉风格对齐原系统 */
  --el-select-border-color-hover: var(--color-primary);
  --el-select-input-color: var(--text-primary);
  --el-select-input-focus-border-color: var(--color-primary);

  :deep(.el-input__wrapper) {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    box-shadow: none !important;
    padding: 2px 10px;
    min-height: 36px;
    transition: all 0.2s;
    cursor: pointer;

    &:hover {
      background: var(--bg-hover);
      border-color: var(--border-color);
    }

    &.is-focus {
      background: var(--bg-hover);
      border-color: var(--color-primary);
      box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-primary) 20%, transparent) !important;
    }

    .el-input__inner {
      color: var(--text-primary) !important;
      font-size: var(--font-size-base) !important;
      font-weight: 500;
      cursor: pointer;
      min-width: 80px;

      &::placeholder {
        color: var(--text-secondary);
      }
    }

    .el-input__suffix {
      color: var(--text-secondary);
    }
  }

  /* 选中后的文本：图标由 #prefix 槽提供，不再额外左移 */
  :deep(.el-select__wrapper),
  :deep(.el-select__prefix),
  :deep(.el-select__selection),
  :deep(.el-select__selected-item) {
    align-items: center;
  }

  :deep(.el-select__selected-item) {
    padding-left: 0;
    line-height: 1.4;
  }

  :deep(.el-select__selection) {
    gap: 8px;
  }

  /* 小箭头图标动画 */
  :deep(.el-input__suffix-inner) {
    transition: transform 0.2s;
  }

  :deep(.el-input__wrapper.is-focus .el-input__suffix-inner) {
    transform: rotate(180deg);
  }
}

.model-selector-prefix {
  display: inline-flex;
  height: 24px;
  align-items: center;
  justify-content: center;
  line-height: 1;

  .el-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
    transform: translateY(1px);
  }

  img,
  svg {
    display: block;
  }
}

/* 与 ChatView 智能对话模型项布局对齐 */
.model-item {
  width: 100%;
  display: flex;
  align-items: center;
  max-width: 340px;
  overflow: hidden;
  position: relative;

  .el-icon {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .model-item-title {
    display: flex;
    align-items: center;
    max-width: 200px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  &.vendor-incomplete {
    overflow: visible;

    .model-item-title {
      opacity: 0.45;
    }

    > .el-icon {
      opacity: 0.45;
    }
  }


  .model-tag-box {
    // max-width: calc(100% - 200px);
    flex: 1;
    margin-left: 10px;
    overflow: hidden;
    white-space: nowrap;

    .scroll-box {
      height: 100%;
      display: flex;
      align-items: center;

    }

    .el-tag {
      font-weight: normal;
    }

    .el-tag+.el-tag {
      margin-left: 4px;
    }
  }
}
</style>

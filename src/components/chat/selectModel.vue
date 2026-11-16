<template>
  <el-dialog
    v-model="visible"
    :title="t('chat.selectModelTitle')"
    width="50%"
    align-center
    append-to-body
    class="select-model-dialog"
  >
    <div class="search-btn">
      <el-input
        v-model="searchModelStr"
        class="select-model-search"
        :placeholder="t('chat.selectModelPlaceholder')"
        clearable
      />
      <el-button class="sidebar-expand-btn" :icon="Plus" :title="t('chat.expandSidebarTitle')" @click="toggleCollapse(true)" />
      <el-button class="sidebar-collapse-btn" :icon="Minus" :title="t('chat.collapseSidebarTitle')" @click="toggleCollapse(false)" />
    </div>
    <div class="common-tags">
      <div>{{ t('chat.commonTags') }}</div>
      <div>
        <template v-for="(item, index) in commonTags" :key="index">
          <el-tag :type="item.type" @click="selectTag(item)">{{ item.name }}</el-tag>
        </template>
      </div>
    </div>
    <el-scrollbar height="50vh" v-loading="loading">
      <el-collapse v-model="activeNames">
        <el-collapse-item v-for="(item, index) in showModelList" :key="index" :name="index + 1">
          <template #title>
            <div style="display: flex;align-items: center;">
              <el-icon :size="16" class="group-icon">
                <svg-icon :iconName="getProviderIcon(item.providerCode)" />
              </el-icon>
              <span>{{ item.label }}</span>
            </div>
          </template>
          <div class="model-box">
            <template v-for="(model, modelInd) in item.children" :key="model.modelId">
              <div class="model-item"
                :class="{ active: model.modelId == selectModelId }"
                @click="onSelectModelClick(item, model.modelId)" v-if="model.show">
                <el-icon :size="16" class="model-provider-icon">
                  <svg-icon :iconName="modelIconName(model)" />
                </el-icon>
                <div class="model-item-title" :title="model.modelName">{{ model.modelName }}</div>
                <div class="model-tag-box" v-if="model.tags?.length">
                  <div class="scroll-box" :title="model.modelLabel">
                    <template v-for="(tag, tagInd) in model.tags" :key="tagInd">
                      <el-tag type="primary" effect="light" size="small" v-if="PRESET_CHIPS.includes(tag)">
                        <el-icon :size="14" :title="tag">
                          <svg-icon :iconName="tag" />
                        </el-icon>
                      </el-tag>
                      <el-tag type="primary" effect="light" size="small" v-else>{{ tag }}</el-tag>
                    </template>
                  </div>
                </div>
                <div class="operation-box">
                  <div class="operation-item">
                    <template v-if="item.alwaysShow">
                      <el-icon :size="18" :title="t('chat.unpinTop')" @click.stop="restorePosit(model, modelInd)">
                        <Download />
                      </el-icon>
                    </template>
                    <template v-else>
                      <el-icon class="pin-top" :size="18" :title="t('chat.pinToTop')"
                        @click.stop="toTop(model, index, modelInd)">
                        <Download />
                      </el-icon>
                    </template>
                  </div>
                </div>
              </div>
            </template>
          </div>
        </el-collapse-item>
      </el-collapse>
      <el-empty v-if="showModelList.length == 0" :description="$t('chat.noModel')">
        <template #extra>
          <span style="font-size:12px;color:var(--text-secondary)">{{ $t('chat.noModelHint') || '请前往 设置 → 模型管理 添加模型' }}</span>
        </template>
      </el-empty>
    </el-scrollbar>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n'
import { Plus, Minus, Download } from '@element-plus/icons-vue';
import { enrichTreeData, groupFlatModels, transformToSelectModelGroups, addPinnedModelId, removePinnedModelId, getPinnedModelIds, type SelectModelGroup, type SelectModelItem } from '@/utils/modelUtils'
import SvgIcon from '@/components/svgIcon/index.vue'

const { t } = useI18n()

/** 与智能对话模型下拉一致：预设能力标签走图标 */
const PRESET_CHIPS = ['视觉', '联网', '推理', '工具', '重排', '嵌入']

async function onSelectModelClick(g: SelectModelGroup, modelId: string | number) {
  selectModelChange(modelId)
}

/** 固定供应商映射 → 与 ModelsView 固定 tab 图标一致 */
const FIXED_ICON: Record<string, string> = { HUIZHI: 'CI', PRIVATE: 'lan', LOCAL: 'local' }

/** 主图标：固定供应商用映射，其余直接用 providerCode 作为图标名（与 ModelsView 非固定供应商逻辑一致） */
function getProviderIcon(providerCode?: string): string {
  if (!providerCode) return 'internet'
  return FIXED_ICON[providerCode] ?? providerCode
}

const emit = defineEmits(['selectModelData']);

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

const visible = ref(false)
const activeNames = ref<number[]>([])
const selectModelId = ref('')
const modelList = ref<SelectModelGroup[]>([])
const loading = ref(false)
const commonTags = ref([
  { name: '代码', type: 'info' },
  { name: '文本', type: 'info' },
  { name: '图像', type: 'info' },
])
const tagText = ref('')
const searchModelStr = ref('')

const showModelList = computed(() => {
  if (searchModelStr.value || tagText.value) {
    const lowerSearchTerm = searchModelStr.value.toLowerCase()
    const lowerTagTerm = tagText.value.toLowerCase()

    return modelList.value.map(group => {
      if (group.alwaysShow) {
        return { ...group }
      }
      let filteredOptions: SelectModelItem[]
      if (lowerSearchTerm && lowerTagTerm) {
        filteredOptions = group.children.filter(option => {
          let num = 0
          if (option.modelName.toLowerCase().includes(lowerSearchTerm)
            || option.modelLabel.toLowerCase().includes(lowerSearchTerm)) {
            num++
          }
          const arr = lowerTagTerm.split(',')
          for (const i of arr) {
            if (option.modelLabel.toLowerCase().includes(i)) {
              num++
              break
            }
          }
          return num == 2
        })
      } else if (lowerSearchTerm) {
        filteredOptions = group.children.filter(option =>
          option.modelName.toLowerCase().includes(lowerSearchTerm)
          || option.modelLabel.toLowerCase().includes(lowerSearchTerm)
        )
      } else {
        filteredOptions = group.children.filter(option => {
          const arr = lowerTagTerm.split(',')
          for (const i of arr) {
            if (option.modelLabel.toLowerCase().includes(i)) {
              return true
            }
          }
          return false
        })
      }
      return { ...group, children: filteredOptions }
    }).filter(group => group.children.length > 0)
  }
  return modelList.value
})

const selectTag = (item: { name: string; type: string }) => {
  item.type = item.type === 'info' ? 'primary' : 'info'
  tagText.value = commonTags.value.filter(i => i.type === 'primary').map(i => i.name).join(',')
}

const restorePosit = (item: SelectModelItem, index: number) => {
  if (!item.oldIndex) return
  const [origGroupIdx, origChildIdx] = item.oldIndex
  // Remove from pinned group
  modelList.value[0].children.splice(index, 1)
  // Restore in original group (offset by 1 for pinned group)
  modelList.value[origGroupIdx + 1].children[origChildIdx].show = true
  // Remove pinned group if empty
  if (modelList.value[0].children.length === 0) {
    modelList.value.shift()
    activeNames.value = activeNames.value.filter(i => i > 0).map(i => i - 1)
  }
  removePinnedModelId(String(item.modelId))
}

const toTop = (item: SelectModelItem, _index: number, childIndex: number) => {
  // Hide in original position
  modelList.value[_index].children[childIndex].show = false
  if (modelList.value[0]?.label === t('chat.pinnedGroup')) {
    modelList.value[0].children.push({ ...item, show: true })
  } else {
    modelList.value.unshift({
      label: t('chat.pinnedGroup'),
      alwaysShow: true,
      children: [{ ...item, show: true }],
    })
    activeNames.value = activeNames.value.map(i => i + 1)
    activeNames.value.unshift(1)
  }
  addPinnedModelId(String(item.modelId))
}

const selectModelChange = (id: string | number) => {
  visible.value = false
  emit('selectModelData', id)
}

const toggleCollapse = (flag: boolean) => {
  if (flag) {
    activeNames.value = modelList.value.map((_, i) => i + 1)
  } else {
    activeNames.value = []
  }
}

const getModelList = async () => {
  loading.value = true
  const invoke = await getInvoke()
  if (!invoke) {
    loading.value = false
    return
  }

  try {
    const tree = await invoke<any[]>('list_models_tree')
    if (Array.isArray(tree) && tree.length > 0) {
      const enriched = enrichTreeData(tree)
      const pinnedIds = getPinnedModelIds()
      modelList.value = transformToSelectModelGroups(enriched, pinnedIds)
      activeNames.value = modelList.value.map((_, i) => i + 1)
      loading.value = false
      return
    }
  } catch { /* fallback to flat list */ }

  try {
    const list = await invoke<any[]>('list_models')
    const enabled = list.filter((m: any) => m.is_enabled !== false)
    if (enabled.length > 0) {
      const grouped = groupFlatModels(enabled)
      const pinnedIds = getPinnedModelIds()
      modelList.value = transformToSelectModelGroups(grouped, pinnedIds)
      activeNames.value = modelList.value.map((_, i) => i + 1)
    }
  } catch (err) {
    console.warn('Failed to load models for selector', err)
  }
  loading.value = false
}

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

function matchIcon(text: string): string | null {
  const lower = text.toLowerCase()
  for (const [keyword, icon] of MODEL_ICON_MAP) {
    if (lower.includes(keyword)) return icon
  }
  return null
}

const modelIconName = (model: any) => {
  return matchIcon(model.modelId) ?? matchIcon(model.modelName) ?? model.modelName
}

const open = () => {
  visible.value = true
  getModelList()
}

defineExpose({ open })
</script>

<style lang="scss" scoped>
.search-btn {
  width: 100%;
  display: flex;
  align-items: center;

  .el-input {
    flex: 1;
  }

  .el-input+.el-button {
    margin-left: 10px;
  }
}

.group-icon {
  margin-right: 6px;
  color: var(--text-secondary, #888);
}

.common-tags {
  width: 100%;
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 6px;
  padding: 10px 0 6px;

  .el-tag {
    cursor: pointer;
  }

  .el-tag+.el-tag {
    margin-left: 4px;
  }
}

.el-collapse {
  border-top: none;
}

.model-box {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 0 15px;

  .model-item {
    width: 100%;
    padding: 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
    cursor: pointer;
    border-radius: 8px;
    position: relative;
    max-width: 100%;

    &.active,
    &:hover:not(.model-item-disabled) {
      background-color: rgba(255,255,255,0.1);
    }

    &.model-item-disabled {
      opacity: 0.45;
      cursor: not-allowed;
    }

    &:hover .operation-box {
      opacity: 1;
    }

    .model-item-logo {
      width: 24px;
      height: 24px;

      .model-img {
        width: 100%;
        height: 100%;
        border-radius: 50%;
      }
    }

    .model-item-title {
      max-width: 200px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .model-tag-box {
      overflow: hidden;
      white-space: nowrap;

      .scroll-box {
        height: 100%;
        display: flex;
        align-items: center;
        color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8);
      }

      .el-tag {
        font-weight: normal;
      }

      .el-tag+.el-tag {
        margin-left: 4px;
      }
    }

    .operation-box {
      position: absolute;
      height: 24px;
      right: 15px;
      opacity: 0;
      transition: opacity .2s;
      display: flex;

      .operation-item {
        height: 100%;
        display: flex;
        align-items: center;

        &:hover .el-icon {
          color: #888;
        }
      }

      .pin-top {
        transform: rotate(180deg);
      }
    }
  }
}

/* 展开/折叠：对齐主题选择弹窗关闭按钮（软底圆角方钮，非主题色实心） */
:deep(.sidebar-expand-btn),
:deep(.sidebar-collapse-btn) {
  width: 32px !important;
  height: 32px !important;
  min-width: 32px !important;
  min-height: 32px !important;
  padding: 0 !important;
  display: inline-flex !important;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary) !important;
  background: var(--cancel-btn-color, rgba(255, 255, 255, 0.08)) !important;
  border: 1px solid var(--btn-secondary-border, transparent) !important;
  border-radius: 8px !important;
  box-shadow: none !important;
  font-weight: 500;
  transition: all 0.18s ease;
}

:deep(.sidebar-expand-btn:hover),
:deep(.sidebar-collapse-btn:hover) {
  color: var(--text-primary) !important;
  background: var(--cancel-btn-hover-color, var(--bg-hover, rgba(255, 255, 255, 0.12))) !important;
  border-color: var(--btn-secondary-hover-border, transparent) !important;
}
</style>

<!-- teleported 到 body 的弹窗头/输入框样式 -->
<style lang="scss">
.select-model-dialog.el-dialog {
  .el-dialog__headerbtn {
    width: 32px !important;
    height: 32px !important;
    top: 12px;
    right: 12px;
    display: inline-flex !important;
    align-items: center;
    justify-content: center;
    border: none;
    background: var(--cancel-btn-color, rgba(255, 255, 255, 0.08)) !important;
    border-radius: 8px !important;
    padding: 0;
    transition: background 0.18s ease, color 0.18s ease;
  }

  .el-dialog__headerbtn:hover {
    background: var(--cancel-btn-hover-color, var(--dropdown-hover-color, var(--bg-hover))) !important;
  }

  .el-dialog__headerbtn .el-dialog__close {
    color: var(--text-secondary) !important;
  }

  .el-dialog__headerbtn:hover .el-dialog__close {
    color: var(--text-primary) !important;
  }

  /* 搜索框：透明底 + 占位符比正文更浅 */
  .select-model-search .el-input__wrapper {
    background: transparent !important;
    box-shadow: none !important;
    border: 1px solid var(--border-color) !important;
  }

  .select-model-search .el-input__inner {
    color: var(--text-primary) !important;
    background: transparent !important;
  }

  .select-model-search .el-input__inner::placeholder,
  .select-model-search .el-input__inner::-webkit-input-placeholder {
    color: color-mix(in srgb, var(--text-primary) 45%, transparent) !important;
    -webkit-text-fill-color: color-mix(in srgb, var(--text-primary) 45%, transparent) !important;
    opacity: 1 !important;
  }

  /* 能力标签：与智能对话下拉一致 */
  .model-tag-box .el-tag.el-tag--primary {
    background: color-mix(in srgb, var(--color-primary) 5%, #ffffff) !important;
    border: 1px solid color-mix(in srgb, var(--color-primary) 8%, transparent) !important;
    color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
  }

  .model-tag-box .el-tag .el-icon,
  .model-tag-box .el-tag svg,
  .model-tag-box .el-tag svg * {
    color: color-mix(in srgb, var(--color-primary) 70%, #94a3b8) !important;
    fill: currentColor !important;
  }
}

html.dark .select-model-dialog.el-dialog .model-tag-box .el-tag.el-tag--primary,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .select-model-dialog.el-dialog .model-tag-box .el-tag.el-tag--primary {
  background: color-mix(in srgb, var(--color-primary) 12%, transparent) !important;
  border: 1px solid color-mix(in srgb, var(--color-primary) 16%, transparent) !important;
  color: color-mix(in srgb, var(--color-primary) 60%, #ffffff) !important;
}

html.dark .select-model-dialog.el-dialog .model-tag-box .el-tag .el-icon,
html.dark .select-model-dialog.el-dialog .model-tag-box .el-tag svg,
html.dark .select-model-dialog.el-dialog .model-tag-box .el-tag svg *,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .select-model-dialog.el-dialog .model-tag-box .el-tag .el-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .select-model-dialog.el-dialog .model-tag-box .el-tag svg,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .select-model-dialog.el-dialog .model-tag-box .el-tag svg * {
  color: color-mix(in srgb, var(--color-primary) 60%, #ffffff) !important;
  fill: currentColor !important;
}
</style>

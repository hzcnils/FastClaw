<template>
  <div class="tool-bind-panel">
    <el-collapse v-model="openPanels">
      <!-- MCP 绑定 -->
      <el-collapse-item name="mcps">
        <template #title>
          <span class="panel-title">{{ t('assistant.bindMcps') }}
            <el-badge v-if="form.mcpIds.length" :value="form.mcpIds.length" class="panel-badge" />
          </span>
        </template>
        <div
          class="tool-list"
          v-loading="loadingMcps"
          element-loading-background="transparent"
        >
          <el-empty v-if="!mcps.length && !loadingMcps" :description="t('assistant.noMcps')" :image-size="40" />
          <div v-for="item in mcps" :key="item.id" class="tool-item">
            <el-checkbox
              :model-value="form.mcpIds.includes(item.id)"
              @change="(v: boolean) => toggleItem(form.mcpIds, item.id, v)"
            >
              <div class="tool-item-info">
                <span class="tool-item-name">{{ item.name }}</span>
                <el-tag size="small" :type="item.status === 'running' ? 'success' : 'info'">
                  {{ item.status === 'running' ? t('assistant.running') : t('assistant.stopped') }}
                </el-tag>
              </div>
            </el-checkbox>
          </div>
        </div>
      </el-collapse-item>

      <!-- Skills 绑定 -->
      <el-collapse-item name="skills">
        <template #title>
          <span class="panel-title">{{ t('assistant.bindSkills') }}
            <el-badge v-if="form.skillIds.length" :value="form.skillIds.length" class="panel-badge" />
          </span>
        </template>
        <div
          class="tool-list"
          v-loading="loadingSkills"
          element-loading-background="transparent"
        >
          <el-empty v-if="!skills.length && !loadingSkills" :description="t('assistant.noSkills')" :image-size="40" />
          <div v-for="item in skills" :key="item.id" class="tool-item">
            <el-checkbox
              :model-value="form.skillIds.includes(item.id)"
              @change="(v: boolean) => toggleItem(form.skillIds, item.id, v)"
            >
              <div class="tool-item-info">
                <span class="tool-item-name">{{ item.name }}</span>
                <span class="tool-item-desc">{{ item.description }}</span>
              </div>
            </el-checkbox>
          </div>
        </div>
      </el-collapse-item>

      <!-- 知识库绑定 -->
      <el-collapse-item name="kbs">
        <template #title>
          <span class="panel-title">{{ t('assistant.bindKbs') }}
            <el-badge v-if="form.knowledgeBaseIds.length" :value="form.knowledgeBaseIds.length" class="panel-badge" />
          </span>
        </template>
        <div
          class="tool-list"
          v-loading="loadingKbs"
          element-loading-background="transparent"
        >
          <el-empty v-if="!kbs.length && !loadingKbs" :description="t('assistant.noKbs')" :image-size="40" />
          <div v-for="item in kbs" :key="item.id" class="tool-item">
            <el-checkbox
              :model-value="form.knowledgeBaseIds.includes(item.id)"
              @change="(v: boolean) => toggleItem(form.knowledgeBaseIds, item.id, v)"
            >
              <div class="tool-item-info">
                <span class="tool-item-name">{{ item.name }}</span>
                <span class="tool-item-desc">{{ t('assistant.docCount', { n: item.doc_count || 0 }) }}</span>
              </div>
            </el-checkbox>
          </div>
        </div>
      </el-collapse-item>
    </el-collapse>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { knowledgeBaseApi } from '@/api/knowledge'

const { t } = useI18n()

interface ToolBindForm {
  mcpIds: string[]
  skillIds: string[]
  knowledgeBaseIds: string[]
}

const props = defineProps<{ modelValue: ToolBindForm }>()
const emit = defineEmits<{ 'update:modelValue': [v: ToolBindForm] }>()

const form = ref<ToolBindForm>({ ...props.modelValue })

function sameIds(a: ToolBindForm, b: ToolBindForm) {
  const eq = (x: string[], y: string[]) => x.length === y.length && x.every((id, i) => id === y[i])
  return eq(a.mcpIds, b.mcpIds)
    && eq(a.skillIds, b.skillIds)
    && eq(a.knowledgeBaseIds, b.knowledgeBaseIds)
}

watch(() => props.modelValue, v => {
  if (sameIds(form.value, v)) return
  form.value = {
    mcpIds: [...v.mcpIds],
    skillIds: [...v.skillIds],
    knowledgeBaseIds: [...v.knowledgeBaseIds],
  }
}, { deep: true })

watch(form, v => {
  if (sameIds(v, props.modelValue)) return
  emit('update:modelValue', {
    mcpIds: [...v.mcpIds],
    skillIds: [...v.skillIds],
    knowledgeBaseIds: [...v.knowledgeBaseIds],
  })
}, { deep: true })

const openPanels = ref<string[]>([])
const mcps = ref<any[]>([])
const skills = ref<any[]>([])
const kbs = ref<any[]>([])
const loadingMcps = ref(false)
const loadingSkills = ref(false)
const loadingKbs = ref(false)

watch(openPanels, async (panels, old) => {
  if (panels.includes('mcps') && !old.includes('mcps') && !mcps.value.length) {
    loadingMcps.value = true
    try {
      mcps.value = await invoke<any[]>('list_mcp_servers', {}) ?? []
    } catch { mcps.value = [] } finally { loadingMcps.value = false }
  }
  if (panels.includes('skills') && !old.includes('skills') && !skills.value.length) {
    loadingSkills.value = true
    try {
      const r = await invoke<any>('list_local_skills', { category: null })
      const list = Array.isArray(r) ? r : (r?.items ?? [])
      skills.value = list.map((item: { id?: string; name: string; description?: string }) => ({
        ...item,
        id: item.id || item.name,
      }))
    } catch { skills.value = [] } finally { loadingSkills.value = false }
  }
  if (panels.includes('kbs') && !old.includes('kbs') && !kbs.value.length) {
    loadingKbs.value = true
    try {
      const list = await knowledgeBaseApi.list({ user_id: 'admin' })
      kbs.value = (Array.isArray(list) ? list : []).map(item => ({
        ...item,
        doc_count: item.file_count ?? 0,
      }))
    } catch { kbs.value = [] } finally { loadingKbs.value = false }
  }
})

function toggleItem(arr: string[], id: string, checked: boolean) {
  if (checked && !arr.includes(id)) arr.push(id)
  else if (!checked) {
    const idx = arr.indexOf(id)
    if (idx > -1) arr.splice(idx, 1)
  }
}
</script>

<style scoped lang="scss">
.tool-bind-panel {
  padding: 4px 0;
}

.panel-title {
  font-size: var(--font-size-base);
  font-weight: 500;
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: 8px;
}

.panel-badge {
  :deep(.el-badge__content) {
    transform: none;
    position: static;
    font-size: 11px;
    padding: 0 5px;
  }
}

.tool-list {
  /* 加载时需足够高度，避免 el-loading 转圈被 overflow 裁切 */
  min-height: 80px;
  max-height: 200px;
  overflow-y: auto;
  padding: 4px 0;
  position: relative;

  :deep(.el-loading-mask) {
    background-color: transparent !important;
  }

  :deep(.el-loading-spinner) {
    margin-top: -21px; /* 默认 -25px 在矮容器里易被裁切 */
  }

  :deep(.el-loading-spinner .circular) {
    width: 36px;
    height: 36px;
  }
}

.tool-item {
  padding: 6px 8px;
  border-radius: 4px;
  transition: background 0.15s;

  &:hover {
    background: var(--bg-hover, rgba(0, 0, 0, 0.04));
  }
}

.tool-item-info {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tool-item-name {
  font-size: var(--font-size-base);
  color: var(--text-primary);
  flex: 1;
}

.tool-item-desc {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 200px;
}
</style>

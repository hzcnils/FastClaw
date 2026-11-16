<template>
  <el-dialog
    class="role-create-dialog"
    :model-value="modelValue"
    :title="mode === 'add' ? t('assistant.createRole') : `${t('assistant.editRole')} — ${initialData?.name ?? ''}`"
    width="680px"
    :append-to-body="true"
    :close-on-click-modal="false"
    destroy-on-close
    @update:model-value="emit('update:modelValue', $event)"
  >
    <el-form ref="formRef" :model="form" :rules="rules" label-width="90px" label-position="right">
      <el-scrollbar height="60vh" view-style="padding-right: 12px; padding-top: 8px;">
        <el-tabs v-model="activeTab" class="dialog-tabs">
          <!-- Tab 1: 基本信息 -->
          <el-tab-pane :label="t('assistant.basicInfo')" name="basic">
            <el-form-item :label="t('assistant.roleName')" prop="name">
              <el-input v-model="form.name" :placeholder="t('assistant.roleNamePlaceholder')" maxlength="50" show-word-limit />
            </el-form-item>

            <el-form-item :label="t('assistant.roleDesc')">
              <el-input v-model="form.description" type="textarea" :rows="2"
                :placeholder="t('assistant.roleDescPlaceholder')" maxlength="200" show-word-limit />
            </el-form-item>

            <el-form-item :label="t('assistant.systemPrompt')" prop="systemPrompt">
              <div class="prompt-area">
                <div class="prompt-templates">
                  <span class="template-label">{{ t('assistant.quickTemplate') }}：</span>
                  <el-button v-for="tpl in TEMPLATES" :key="tpl.name" size="small" text
                    @click="applyTemplate(tpl)">{{ tpl.name }}</el-button>
                </div>
                <el-input v-model="form.systemPrompt" type="textarea" :rows="6"
                  :placeholder="t('assistant.systemPromptPlaceholder')" maxlength="4000" show-word-limit />
              </div>
            </el-form-item>

            <el-form-item :label="t('assistant.defaultModel')">
              <el-select v-model="form.defaultModel" clearable :placeholder="t('assistant.inheritDefaultModel')" style="width: 100%">
                <el-option v-for="m in availableModels" :key="m.id" :value="m.model_id" :label="m.display_name" />
              </el-select>
            </el-form-item>

            <el-form-item label="Temperature">
              <div class="slider-row">
                <el-slider v-model="tempValue" :min="0" :max="200" :step="1"
                  style="flex: 1;" :format-tooltip="(v) => (v / 100).toFixed(2)" />
                <span class="slider-value">{{ (tempValue / 100).toFixed(2) }}</span>
              </div>
            </el-form-item>
          </el-tab-pane>

          <!-- Tab 2: 工具绑定 -->
          <el-tab-pane :label="t('assistant.toolBinding')" name="tools">
            <ToolBindPanel v-model="toolBindings" />
          </el-tab-pane>

          <!-- Tab 3: 高级设置 -->
          <el-tab-pane :label="t('assistant.advancedSettings')" name="advanced">
            <el-form-item :label="t('assistant.contextTurns')">
              <el-input-number v-model="form.contextTurns" :min="1" :max="20" :step="1" />
              <span style="margin-left: 8px; font-size: var(--font-size-sm); color: var(--text-secondary);">{{ t('assistant.turns') }}</span>
            </el-form-item>

            <el-form-item :label="t('assistant.allowPublish')">
              <el-switch v-model="form.allowPublish" />
              <span style="margin-left: 8px; font-size: var(--font-size-sm); color: var(--text-secondary);">
                {{ t('assistant.allowPublishHint') }}
              </span>
            </el-form-item>

            <el-form-item :label="t('assistant.notes')">
              <el-input v-model="form.notes" type="textarea" :rows="3"
                :placeholder="t('assistant.notesPlaceholder')" />
            </el-form-item>
          </el-tab-pane>
        </el-tabs>
      </el-scrollbar>
    </el-form>

    <template #footer>
      <el-button class="theme-action theme-action--secondary" @click="emit('update:modelValue', false)">{{ t('common.cancel') }}</el-button>
      <el-button class="theme-action theme-action--primary" type="primary" :loading="saving" @click="handleSave">{{ t('assistant.confirmSave') }}</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessageBox } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'
import type { Role, CreateRoleData } from '@/types/assistant'
import ToolBindPanel from './ToolBindPanel.vue'

const { t } = useI18n()

const props = defineProps<{
  modelValue: boolean
  mode: 'add' | 'edit'
  initialData?: Role | null
}>()

const emit = defineEmits<{
  'update:modelValue': [v: boolean]
  success: [role: CreateRoleData & { id?: string }]
}>()

const TEMPLATES = [
    { get name() { return t('assistant.tpl_work') }, content: '你是一位专业、高效的工作助手。请用结构化的方式回答问题，善用列表和摘要，确保输出清晰、准确、可执行。' },
    { get name() { return t('assistant.tpl_writing') }, content: '你是一位富有创意的写作助手，文字细腻、表达生动。请协助用户进行文章写作、润色修改和创意构思，注重文字的美感与逻辑性。' },
    { get name() { return t('assistant.tpl_coding') }, content: '你是一位经验丰富的编程专家，精通多种编程语言。请提供可运行的代码示例，附带简要解释，并指出潜在的问题和最佳实践。' },
    { get name() { return t('assistant.tpl_service') }, content: '你是一位礼貌、亲切的客服助手。请用温和的语气回答用户问题，解答准确且耐心，遇到不确定的问题时请诚实告知，避免给出错误信息。' },
  ];

const formRef = ref()
const saving = ref(false)
const activeTab = ref('basic')
const availableModels = ref<any[]>([])
const tempValue = ref(70) // 0.70 * 100

const form = reactive({
  name: '',
  description: '',
  systemPrompt: '',
  defaultModel: '',
  contextTurns: 10,
  allowPublish: true,
  notes: '',
})

const toolBindings = ref({
  mcpIds: [] as string[],
  skillIds: [] as string[],
  knowledgeBaseIds: [] as string[],
})

const rules = {
  name: [{ required: true, whitespace: true, message: t('assistant.roleNameRequired'), trigger: 'blur' }],
  systemPrompt: [{ required: true, whitespace: true, message: t('assistant.systemPromptRequired'), trigger: 'blur' }],
}

watch(() => props.modelValue, async (visible) => {
  if (!visible) return
  activeTab.value = 'basic'
  if (props.mode === 'edit' && props.initialData) {
    const d = props.initialData
    form.name = d.name
    form.description = d.description ?? ''
    form.systemPrompt = d.systemPrompt
    form.defaultModel = d.defaultModel ?? ''
    form.contextTurns = d.contextTurns
    form.allowPublish = d.allowPublish
    form.notes = d.notes ?? ''
    tempValue.value = d.temperature != null ? Math.round(d.temperature * 100) : 70
    toolBindings.value = {
      mcpIds: d.mcps?.map(m => m.mcpId) ?? [],
      skillIds: d.skills?.map(s => s.skillId) ?? [],
      knowledgeBaseIds: d.knowledgeBases?.map(k => k.kbId) ?? [],
    }
  } else {
    resetForm()
  }
})

onMounted(async () => {
  try {
    const r = await invoke<any>('list_models', {})
    availableModels.value = r?.items ?? r ?? []
  } catch { availableModels.value = [] }
})

function resetForm() {
  form.name = ''
  form.description = ''
  form.systemPrompt = ''
  form.defaultModel = ''
  form.contextTurns = 10
  form.allowPublish = true
  form.notes = ''
  tempValue.value = 70
  toolBindings.value = { mcpIds: [], skillIds: [], knowledgeBaseIds: [] }
}

async function applyTemplate(tpl: { name: string; content: string }) {
  if (form.systemPrompt && form.systemPrompt.trim()) {
    try {
      await ElMessageBox.confirm(t('assistant.promptOverwriteMsg'), t('common.confirm'), {
        confirmButtonText: t('assistant.overwrite'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
      })
    } catch { return }
  }
  form.systemPrompt = tpl.content
}

async function handleSave() {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  saving.value = true
  try {
    const data: CreateRoleData & { id?: string } = {
      id: props.mode === 'edit' ? props.initialData?.id : undefined,
      name: form.name.trim(),
      description: form.description.trim() || undefined,
      systemPrompt: form.systemPrompt.trim(),
      defaultModel: form.defaultModel || undefined,
      temperature: tempValue.value / 100,
      contextTurns: form.contextTurns,
      allowPublish: form.allowPublish,
      notes: form.notes.trim() || undefined,
      ...toolBindings.value,
    }
    emit('success', data)
  } finally {
    saving.value = false
  }
}
</script>

<style scoped lang="scss">
.dialog-tabs {
  :deep(.el-tabs__nav-wrap) {
    border-bottom: 1px solid var(--border-color);
  }
}

.prompt-area {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.prompt-templates {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.template-label {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.slider-row {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
}

.slider-value {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  width: 36px;
  text-align: right;
}

:deep(.el-input__wrapper) {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
}

:deep(.el-textarea__inner) {
  background: transparent !important;
  border:1px solid var(--border-color) !important;
  box-shadow: none !important;

  &:hover, &:focus {
    box-shadow: none !important;
  }
}

:deep(.el-select__wrapper) {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
}

:deep(.el-input-number) {
  .el-input__wrapper {
    background: transparent !important;
    border-color: var(--border-color) !important;
  }
}

:deep(.theme-action) {
  min-width: 76px;
  height: 36px;
  padding: 0 16px;
  border: 1px solid transparent;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-base);
  font-weight: 600;
  transition: all 0.18s ease;
}

.theme-action--secondary {
  min-width: 76px;
  height: 36px;
  padding: 0 16px;
  border-radius: 4px;
  font-size: var(--font-size-base);
  font-weight: 600;
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  border-color: var(--btn-secondary-border) !important;

  &:hover {
    background: var(--cancel-btn-hover-color) !important;
    color: var(--btn-secondary-hover-text) !important;
    border-color: var(--btn-secondary-hover-border) !important;
  }
}

:deep(.theme-action--primary) {
 display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 12px;
  border-radius: 4px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border-color: var(--btn-primary-border, transparent);
  font-size: var(--font-size-md);
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}
</style>

<style lang="scss">
/* 提示文字比正文浅一档；字数统计与提示文字同色 */
.role-create-dialog,
.role-create-dialog .el-dialog {
  --role-field-hint: color-mix(in srgb, var(--text-primary) 55%, transparent);
}

.role-create-dialog .el-input__inner::placeholder,
.role-create-dialog .el-textarea__inner::placeholder {
  color: var(--role-field-hint) !important;
  -webkit-text-fill-color: var(--role-field-hint) !important;
  opacity: 1 !important;
}

.role-create-dialog .el-input__suffix-inner,
.role-create-dialog .el-input__suffix-inner .el-input__count,
.role-create-dialog .el-textarea .el-input__count,
.role-create-dialog .el-textarea .el-input__count-inner {
  color: var(--role-field-hint) !important;
  -webkit-text-fill-color: var(--role-field-hint) !important;
  background: transparent !important;
}

/* 关闭按钮对齐主题选择弹窗 .theme-modal__close */
.role-create-dialog.el-dialog .el-dialog__headerbtn,
.role-create-dialog .el-dialog__headerbtn {
  width: 32px !important;
  height: 32px !important;
  top: 12px !important;
  right: 16px !important;
  display: inline-flex !important;
  align-items: center !important;
  justify-content: center !important;
  padding: 0 !important;
  color: var(--text-secondary) !important;
  background: transparent !important;
  border: none !important;
  border-radius: 8px !important;
  transition: all 0.18s ease;
}

.role-create-dialog.el-dialog .el-dialog__headerbtn .el-dialog__close,
.role-create-dialog .el-dialog__headerbtn .el-dialog__close {
  color: var(--text-secondary) !important;
}

.role-create-dialog.el-dialog .el-dialog__headerbtn:hover,
.role-create-dialog .el-dialog__headerbtn:hover {
  color: var(--text-primary) !important;
  background: var(--bg-hover) !important;
}

.role-create-dialog.el-dialog .el-dialog__headerbtn:hover .el-dialog__close,
.role-create-dialog .el-dialog__headerbtn:hover .el-dialog__close {
  color: var(--text-primary) !important;
}
</style>

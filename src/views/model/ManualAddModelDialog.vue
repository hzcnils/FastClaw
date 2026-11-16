<script setup lang="ts">
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import TagSelectDialog from './TagSelectDialog.vue'
import { saveRemoteModels } from '@/modules/models/api'
import type { RemoteModel } from '@/modules/models/types/contracts'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  providerCode: string
  providerName: string
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'saved'): void
}>()

const form = ref({
  model_name: '',
  context_length: 4096,
  max_output: null as number | null,
  model_tags: [] as string[],
  extra_config: '',
})

const showTagSelectDialog = ref(false)
const saving = ref(false)

watch(() => props.visible, (val) => {
  if (!val) return
  form.value = {
    model_name: '',
    context_length: 4096,
    max_output: null,
    model_tags: [],
    extra_config: '',
  }
  showTagSelectDialog.value = false
})

function close() {
  emit('update:visible', false)
}

async function saveManualModel() {
  if (!props.providerCode || !form.value.model_name.trim() || saving.value) return
  saving.value = true
  try {
    const extraConfig = form.value.extra_config.trim()
      ? (() => { try { return JSON.parse(form.value.extra_config.trim()) } catch { return null } })()
      : null

    await saveRemoteModels(props.providerCode, [{
        id: form.value.model_name.trim(),
        provider_code: props.providerCode,
        model_name: form.value.model_name.trim(),
        model_type: 'chat',
        context_length: form.value.context_length,
        max_output: form.value.max_output ?? undefined,
        tags: form.value.model_tags.length > 0 ? form.value.model_tags : undefined,
        extra_config: extraConfig ? JSON.stringify(extraConfig) : undefined,
        is_enabled: true,
      } satisfies RemoteModel])
    emit('saved')
    close()
    ElMessage.success(t('modelManage.addModelSuccess'))
  } catch (e: any) {
    console.error('Manual save model failed:', e)
    ElMessage.error(t('modelManage.saveModelFail', { msg: e.message || String(e) }))
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <!-- 与 EditModelDialog 完全同一套：无 Teleport、内联 fixed，打包环境已验证可用 -->
  <div
    v-if="visible"
    style="position:fixed;inset:0;z-index:2000;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,0.5)"
  >
    <div
      style="background:var(--theme-dailog-color);padding:20px;border-radius:12px;width:520px;max-height:85vh;overflow-y:auto;box-shadow:0 24px 70px rgba(0,0,0,0.3);box-sizing:border-box"
      @click.stop
    >
      <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px">
        <h3 style="margin:0;font-size:17px;font-weight:600;">
          {{ $t('modelManage.manualAddTitle', { name: providerName || '' }) }}
        </h3>
        <button type="button" class="modal-close" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>

      <div style="margin-bottom:12px">
        <label style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.modelName') }}</label>
        <input
          v-model="form.model_name"
          class="custom-input"
          :placeholder="$t('modelManage.modelNamePlaceholder')"
        />
      </div>

      <div style="margin-bottom:12px">
        <label style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.contextLength') }}</label>
        <div class="number-input-wrap">
          <input
            v-model.number="form.context_length"
            type="number"
            class="custom-input"
            :placeholder="$t('modelManage.contextLengthPlaceholder')"
          />
          <div class="number-arrows">
            <button type="button" class="number-arrow" @click="form.context_length = (form.context_length || 0) + 1">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            </button>
            <button type="button" class="number-arrow" @click="form.context_length = Math.max(0, (form.context_length || 0) - 1)">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9"></polyline>
              </svg>
            </button>
          </div>
        </div>
      </div>

      <div style="margin-bottom:12px">
        <label style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.maxOutput') }}</label>
        <div class="number-input-wrap">
          <input
            v-model.number="form.max_output"
            type="number"
            class="custom-input"
            :placeholder="$t('modelManage.maxOutputPlaceholder')"
          />
          <div class="number-arrows">
            <button type="button" class="number-arrow" @click="form.max_output = (form.max_output || 0) + 1">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            </button>
            <button type="button" class="number-arrow" @click="form.max_output = Math.max(0, (form.max_output || 0) - 1)">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9"></polyline>
              </svg>
            </button>
          </div>
        </div>
      </div>

      <div class="form-group" style="margin-bottom:12px">
        <label class="form-label" style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.label') }}</label>
        <div class="tag-input-area">
          <el-tag v-for="(tag, i) in form.model_tags" :key="i" closable size="small" @close="form.model_tags.splice(i, 1)">
            {{ tag }}
          </el-tag>
          <el-button size="small" class="tag-add-btn" @click="showTagSelectDialog = true">
            {{ $t('agent.addTag') }}
          </el-button>
        </div>
      </div>

      <div style="margin-bottom:12px">
        <label style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.extConfig') }}</label>
        <textarea
          v-model="form.extra_config"
          rows="5"
          class="custom-input"
          :placeholder="$t('modelManage.extraConfigPlaceholder')"
          style="resize:vertical;font-family:monospace"
        />
      </div>

      <div style="display:flex;justify-content:flex-end;align-items:center;gap:15px;margin-top:16px">
        <button type="button" class="btn-cancel" @click="close">{{ $t('common.cancel') }}</button>
        <button
          type="button"
          class="btn-confirm"
          :disabled="!form.model_name.trim() || saving"
          @click="saveManualModel"
        >{{ $t('common.confirm') }}</button>
      </div>
    </div>

    <TagSelectDialog
      v-if="showTagSelectDialog"
      v-model:visible="showTagSelectDialog"
      v-model:modelValue="form.model_tags"
    />
  </div>
</template>

<style scoped>
/* 复用 ModelsView 里 .custom-input 时，本组件内再写一份兜底 */
.custom-input {
  width: 100%;
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  box-sizing: border-box;
  outline: none;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--font-size-md, 14px);
}

.modal-close {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 5px;
}

.modal-close:hover {
  color: var(--text-primary);
  background: var(--dropdown-hover-color, var(--bg-hover));
}

.tag-input-area {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.tag-add-btn {
  font-size: 12px;
  padding: 2px 8px;
}
.tag-add-btn:hover{
    background :var(--dropdown-hover-color);
    border:1px solid var(--border-color);
  }
.number-input-wrap {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
}

.number-input-wrap input[type='number'] {
  width: 100%;
  padding-right: 28px;
  -moz-appearance: textfield;
}

.number-input-wrap input[type='number']::-webkit-inner-spin-button,
.number-input-wrap input[type='number']::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.number-arrows {
  position: absolute;
  right: 1px;
  top: 1px;
  bottom: 1px;
  width: 26px;
  display: flex;
  flex-direction: column;
  opacity: 0;
  transition: opacity 0.2s;
  border-radius: 0 5px 5px 0;
  overflow: hidden;
}

.number-input-wrap:hover .number-arrows {
  opacity: 1;
}

.number-arrow {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--text-primary);
  cursor: pointer;
  padding: 0;
}

.number-arrow:hover {
  background: rgba(128, 128, 128, 0.2);
}

.btn-cancel,
.btn-confirm {
  width: 64px;
  height: 37px;
  padding: 0;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md, 14px);
  font-weight: 500;
  box-sizing: border-box;
}

.btn-cancel {
  color: var(--btn-secondary-text);
  background: var(--cancel-btn-color);
  border: 1px solid var(--btn-secondary-border);
}

.btn-cancel:hover {
  background: var(--cancel-btn-hover-color);
  color: var(--btn-secondary-hover-text);
  border-color: var(--btn-secondary-hover-border);
}

.btn-confirm {
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
}

.btn-confirm:hover:not(:disabled) {
  background: var(--btn-primary-hover-bg, var(--color-primary));
  color: var(--btn-primary-hover-text, #fff);
  border-color: var(--btn-primary-hover-border, transparent);
}

.btn-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>

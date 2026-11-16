<script setup lang="ts">
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import TagSelectDialog from './TagSelectDialog.vue'
import { updateModels } from '@/modules/models/api'
import type { ModelUpdatePatch } from '@/modules/models/types/contracts'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  model: {
    id: string
    display_name: string
    model_label: string
    context_window?: number | string | null
    context_length?: number | string
    mxcs?: any
  } | null
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'saved', data: { id: string; display_name: string; model_label: string | null; context_window: number | string; mxcs: any }): void
}>()

const form = ref({
  id: '',
  display_name: '',
  model_tags: [] as string[],
  context_length: '' as string | number,
  mxcs: '',
})

const mxcsError = ref('')
const showTagSelectDialog = ref(false)

function parseModelLabels(label: string): string[] {
  if (!label) return []
  return label.split(/[,，]/).map((s) => s.trim()).filter(Boolean)
}

function safeParseModelLabels(label?: string) {
  try {
    return parseModelLabels(label || '')
  } catch {
    return []
  }
}

function removeEditModelTag(index: number) {
  form.value.model_tags.splice(index, 1)
}

watch(() => props.visible, (val) => {
  if (val && props.model) {
    const m = props.model
    let mxcsFormatted = ''
    if (m.mxcs != null && m.mxcs !== '') {
      try {
        if (typeof m.mxcs === 'object') {
          const cache = new Set()
          mxcsFormatted = JSON.stringify(m.mxcs, (key, value) => {
            if (typeof value === 'object' && value !== null) {
              if (cache.has(value)) return '[循环引用对象]'
              cache.add(value)
            }
            return value
          }, 2)
        } else if (typeof m.mxcs === 'string') {
          const parsed = JSON.parse(m.mxcs)
          const cache = new Set()
          mxcsFormatted = JSON.stringify(parsed, (key, value) => {
            if (typeof value === 'object' && value !== null) {
              if (cache.has(value)) return '[循环引用对象]'
              cache.add(value)
            }
            return value
          }, 2)
        }
      } catch {
        mxcsFormatted = String(m.mxcs ?? '')
      }
    }

    form.value = {
      id: m.id,
      display_name: m.display_name || m.model_label || '',
      model_tags: safeParseModelLabels(m.model_label),
      context_length: m.context_window ?? m.context_length ?? '',
      mxcs: mxcsFormatted,
    }
    mxcsError.value = ''
  }
})

function close() {
  emit('update:visible', false)
}

async function confirmEditModel() {
  try {
    const mxcs = String(form.value.mxcs ?? '').trim()
    if (mxcs) {
      try { JSON.parse(mxcs) } catch {
        mxcsError.value = t('modelManage.jsonInvalidSave')
        return
      }
    }
    const modelLabel = form.value.model_tags.length > 0
      ? form.value.model_tags.join(',')
      : null
    const updates: ModelUpdatePatch = {
      display_name: form.value.display_name || null,
      model_label: modelLabel,
      mxcs: mxcs || null,
    }
    if (form.value.context_length !== '' && form.value.context_length !== null && form.value.context_length !== undefined) {
      updates.context_window = Number(form.value.context_length)
    }
    await updateModels([form.value.id], updates)
    emit('saved', {
      id: form.value.id,
      display_name: form.value.display_name,
      model_label: modelLabel,
      context_window: form.value.context_length,
      mxcs: mxcs,
    })
    close()
    ElMessage.success(t('modelManage.updateModelSuccess'))
  } catch (e: any) {
    console.warn('Save model failed:', e)
    ElMessage.error(t('modelManage.updateFailed', { msg: e.message || String(e) }))
  }
}
</script>

<template>
  <div v-if="visible"
    style="position:fixed;inset:0;z-index:2000;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,0.5)">
    <div
      style="background:var(--theme-dailog-color);padding:20px 20px 20px 20px;border-radius:12px;width:520px;max-height:85vh;overflow-y:auto;box-shadow:0 24px 70px rgba(0,0,0,0.3)">
      <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px">
        <h3 style="margin:0;font-size:17px;font-weight:600;">{{ $t('modelManage.editModelTitle') }}</h3>
        <button class="modal-close" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
      <div style="margin-bottom:12px">
        <label style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.displayName') }}</label>
        <input :value="form.display_name" @input="form.display_name = ($event.target as HTMLInputElement).value"
          :placeholder="$t('modelManage.displayNamePlaceholder')" class="custom-input" />
      </div>

      <div class="form-group">
        <label class="form-label">{{ $t('modelManage.label') }}</label>
        <div class="tag-input-area">
          <el-tag v-for="(tag, i) in form.model_tags" :key="i" closable size="small"
            @close="removeEditModelTag(i)">
            {{ tag }}
          </el-tag>
          <el-button size="small" class="tag-add-btn" @click="showTagSelectDialog = true">
            {{ $t('agent.addTag') }}
          </el-button>
        </div>
      </div>

      <div style="margin-bottom:12px;margin-top:12px">
        <label style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.contextLength') }}</label>
        <div class="number-input-wrap">
          <input :value="form.context_length" @input="form.context_length = ($event.target as HTMLInputElement).value"
            type="number" :placeholder="$t('modelManage.contextLengthPlaceholder')" class="custom-input" />
          <div class="number-arrows">
            <button class="number-arrow number-arrow-up" type="button"
              @click="form.context_length = (Number(form.context_length) || 0) + 1">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none"
                stroke-linecap="round" stroke-linejoin="round">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            </button>
            <button class="number-arrow number-arrow-down" type="button"
              @click="form.context_length = Math.max(0, (Number(form.context_length) || 0) - 1)">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none"
                stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9"></polyline>
              </svg>
            </button>
          </div>
        </div>
      </div>

      <div style="margin-bottom:12px">
        <label style="display:block;font-size:13px;margin-bottom:4px">{{ $t('modelManage.extConfig') }}</label>
        <textarea :value="form.mxcs" @input="form.mxcs = ($event.target as HTMLTextAreaElement).value" rows="8"
          class="custom-input" />
      </div>
      <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:16px">
        <button @click="close" class="button-cancel">{{ $t('common.cancel') }}</button>
        <button @click="confirmEditModel" class="button-confirm">{{ $t('common.confirm') }}</button>
      </div>
    </div>
  </div>

  <TagSelectDialog
    v-model:visible="showTagSelectDialog"
    v-model:modelValue="form.model_tags"
  />
</template>

<style scoped>
.modal-close {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-close:hover {
  color: var(--text-primary);
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

.number-input-wrap input[type="number"] {
  width: 100%;
  padding-right: 28px;
  -moz-appearance: textfield;
}

.number-input-wrap input[type="number"]::-webkit-inner-spin-button,
.number-input-wrap input[type="number"]::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.number-input-wrap .number-arrows {
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

.number-input-wrap .number-arrow {
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

.number-input-wrap .number-arrow:hover {
  background: rgba(128, 128, 128, 0.2);
}

.number-input-wrap .number-arrow svg {
  width: 12px;
  height: 12px;
}
</style>

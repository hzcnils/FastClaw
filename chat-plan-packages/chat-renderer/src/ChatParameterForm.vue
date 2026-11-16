<script setup lang="ts">
import { computed } from 'vue'
import type { ChatParameterField, ChatParameterValues } from './parameterForm'

const props = withDefaults(defineProps<{
  fields: ChatParameterField[]
  modelValue: ChatParameterValues
  title?: string
  description?: string
  submitting?: boolean
  error?: string
  submitLabel?: string
  cancelLabel?: string
}>(), { submitLabel: '确认并发送', cancelLabel: '取消' })
const emit = defineEmits<{
  (event: 'update:modelValue', value: ChatParameterValues): void
  (event: 'submit', value: ChatParameterValues): void
  (event: 'cancel'): void
}>()
const incomplete = computed(() => props.fields.some(field => field.required && !props.modelValue[field.key]?.trim()))
function submit() {
  if (props.submitting || incomplete.value) return
  emit('submit', Object.fromEntries(props.fields.map(field => [field.key, (props.modelValue[field.key] || '').trim()])))
}
</script>

<template>
  <form class="jcr-parameter-form" :aria-busy="submitting" @submit.prevent="submit">
    <strong v-if="title">{{ title }}</strong>
    <p v-if="description" class="jcr-parameter-form__description">
      <svg class="jcr-parameter-form__notice-icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><circle cx="10" cy="10" r="8" /><path d="M10 5.5v5M10 13.5v1" /></svg>
      <span>{{ description }}</span>
    </p>
    <label v-for="field in fields" :key="field.key" class="jcr-parameter-form__field">
      <span>{{ field.label }}</span>
      <input :type="field.type || 'text'" :value="modelValue[field.key] || ''"
        :placeholder="field.placeholder" :required="field.required" :disabled="submitting"
        autocomplete="off" :spellcheck="false"
        @input="emit('update:modelValue', { ...modelValue, [field.key]: ($event.target as HTMLInputElement).value })" />
    </label>
    <p v-if="error" class="jcr-parameter-form__error" role="alert">{{ error }}</p>
    <div class="jcr-parameter-form__actions">
      <button type="submit" :disabled="submitting || incomplete">{{ submitLabel }}</button>
      <button type="button" :disabled="submitting" @click="emit('cancel')">{{ cancelLabel }}</button>
    </div>
  </form>
</template>

<style scoped>
.jcr-parameter-form { --parameter-border: var(--jcr-parameter-border, #ffdb77); display: grid; gap: 10px; padding: 16px; border: 1px solid var(--parameter-border); border-radius: 8px; background: var(--jcr-parameter-bg, color-mix(in srgb, #fff3c4 35%, var(--jcr-bg, #fff))); color: var(--jcr-fg, #303133); min-width: 0; }
.jcr-parameter-form__description { display: flex; align-items: flex-start; gap: 8px; margin: 0 0 2px; color: var(--jcr-parameter-notice, #c86a10); font-size: 13px; line-height: 1.6; }
.jcr-parameter-form__notice-icon { flex: 0 0 16px; width: 16px; height: 16px; margin-top: 2px; color: #e6a23c; }
.jcr-parameter-form__field { display: grid; grid-template-columns: 68px minmax(0, 1fr); align-items: center; gap: 12px; font-size: 13px; }
.jcr-parameter-form__field > span { white-space: nowrap; }
.jcr-parameter-form__field input { box-sizing: border-box; width: 100%; min-width: 0; height: 34px; border: 1px solid var(--parameter-border); border-radius: 5px; background: var(--jcr-bg, #fff); color: inherit; padding: 6px 12px; font: inherit; }
.jcr-parameter-form__field input:focus { outline: 2px solid var(--jcr-primary, #409eff); outline-offset: 1px; }
.jcr-parameter-form__actions { display: flex; flex-wrap: wrap; gap: 8px; }
.jcr-parameter-form__actions button { border: 1px solid var(--jcr-border, #dcdfe6); border-radius: 5px; padding: 7px 16px; font: inherit; font-size: 13px; color: inherit; background: var(--jcr-bg, #fff); cursor: pointer; }
.jcr-parameter-form__actions button[type=submit] { background: var(--jcr-primary, #409eff); border-color: transparent; color: white; }
.jcr-parameter-form__actions button:disabled { opacity: .5; cursor: not-allowed; }
.jcr-parameter-form__error { color: var(--jcr-danger, #dc2626); margin: 0; font-size: 13px; }
</style>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { addProvider as createProvider } from '@/modules/models/api'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'saved'): void
}>()

const form = ref({
  name: '',
  provider_type: 'openai',
  base_url: '',
  api_key: '',
  source_category: 'online',
})

const showKey = ref(false)

function close() {
  emit('update:visible', false)
  form.value = { name: '', provider_type: 'openai', base_url: '', api_key: '', source_category: 'online' }
}

async function addProvider() {
  try {
    await createProvider({
      name: form.value.name,
      providerType: form.value.provider_type,
      baseUrl: form.value.base_url,
      apiKey: form.value.api_key || null,
      sourceCategory: form.value.source_category,
    })
    emit('saved')
    close()
  } catch (e) {
    console.warn('Add provider failed:', e)
  }
}
</script>

<template>
  <div v-if="visible" class="overlay">
    <div class="dialog">
      <div class="dialog-header">
        <h3>{{ $t('modelManage.addProviderTitle') }}</h3>
        <button class="close-btn" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
      <div class="form-group">
        <input v-model="form.name" :placeholder="$t('modelManage.namePlaceholder')" />
      </div>
      <div class="form-group">
        <el-select v-model="form.provider_type" style="width: 100%;">
          <el-option value="openai" :label="$t('modelManage.openaiCompat')" />
          <el-option value="anthropic" label="Anthropic" />
        </el-select>
      </div>
      <div class="form-group">
        <el-select v-model="form.source_category" style="width: 100%;">
          <el-option value="online" :label="$t('modelManage.sourceOnline')" />
        </el-select>
      </div>
      <div class="form-group">
        <input v-model="form.base_url" :placeholder="$t('modelManage.apiAddrPlaceholder')" />
      </div>
      <div class="form-group password-input-wrap">
        <input v-model="form.api_key" :type="showKey ? 'text' : 'password'" :placeholder="$t('modelManage.apiKeyOptional')" />
        <button type="button" class="eye-btn" @click="showKey = !showKey" :title="showKey ? $t('modelManage.hideKey') : $t('modelManage.showKey')">
          <svg v-if="!showKey" viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <path d="M1 12 C1 12 5 4 12 4 C19 4 23 12 23 12 C23 12 19 20 12 20 C5 20 1 12 1 12 Z"></path>
            <circle cx="12" cy="12" r="3"></circle>
          </svg>
          <svg v-else viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"></path>
            <line x1="1" y1="1" x2="23" y2="23"></line>
          </svg>
        </button>
      </div>
      <div class="actions">
        <el-button class="btn-cancel" @click="close">{{ $t('common.cancel') }}</el-button>
        <el-button class="btn-confirm" type="primary" @click="addProvider">{{ $t('common.confirm') }}</el-button>
      </div>
    </div>
  </div>
</template>
<style src="./dialog-common.css"></style>

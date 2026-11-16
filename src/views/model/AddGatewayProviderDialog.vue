<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { addProvider } from '@/modules/models/api'
import type { RawGatewayResponse } from '@/modules/models/types/contracts'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  gateways: RawGatewayResponse[]
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
  gateway_id: '',
})

function close() {
  emit('update:visible', false)
  form.value = { name: '', provider_type: 'openai', base_url: '', api_key: '', gateway_id: '' }
}

async function addGatewayProvider() {
  if (!form.value.name.trim() || !form.value.gateway_id) return
  try {
    await addProvider({
      name: form.value.name,
      providerType: form.value.provider_type,
      baseUrl: form.value.base_url || 'https://api.example.com/v1',
      apiKey: form.value.api_key || null,
      isLocal: false,
      sourceCategory: 'online',
      gatewayId: form.value.gateway_id,
    })
    emit('saved')
    close()
    ElMessage.success(t('modelManage.operationSuccess'))
  } catch (e: any) {
    ElMessage.error(t('modelManage.operationFailed', { msg: e.message || String(e) }))
  }
}
</script>

<template>
  <div v-if="visible" class="overlay">
    <div class="dialog">
      <div class="dialog-header">
        <h3>{{ $t('modelManage.addGatewayProviderTitle') }}</h3>
        <button class="close-btn" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
      <div class="form-group">
        <el-select v-model="form.gateway_id" style="width: 100%;"
          :placeholder="$t('modelManage.selectGateway')">
          <el-option v-for="gw in gateways" :key="gw.id" :label="gw.name" :value="gw.id" />
        </el-select>
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
        <input v-model="form.base_url" :placeholder="$t('modelManage.apiAddrPlaceholder')" />
      </div>
      <div class="form-group">
        <input v-model="form.api_key" :placeholder="$t('modelManage.apiKeyOptional')" type="password" />
      </div>
      <div class="actions">
        <el-button class="btn-cancel" @click="close">{{ $t('common.cancel') }}</el-button>
        <el-button class="btn-confirm" type="primary" @click="addGatewayProvider">{{ $t('common.confirm') }}</el-button>
      </div>
    </div>
  </div>
</template>
<style src="./dialog-common.css"></style>

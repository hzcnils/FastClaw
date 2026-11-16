<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { addGateway as createGateway } from '@/modules/models/api'

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
  description: '',
})

function close() {
  emit('update:visible', false)
  form.value = { name: '', description: '' }
}

async function addGateway() {
  if (!form.value.name.trim()) return
  try {
    await createGateway({
      name: form.value.name,
      description: form.value.description || null,
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
        <h3>{{ $t('modelManage.addGatewayTitle') }}</h3>
        <button class="close-btn" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
      <div class="form-group">
        <input v-model="form.name" :placeholder="$t('modelManage.gatewayNamePlaceholder')" />
      </div>
      <div class="form-group">
        <input v-model="form.description" :placeholder="$t('modelManage.gatewayDescPlaceholder')" />
      </div>
      <div class="actions">
        <el-button class="btn-cancel" @click="close">{{ $t('common.cancel') }}</el-button>
        <el-button class="btn-confirm" type="primary" @click="addGateway">{{ $t('common.confirm') }}</el-button>
      </div>
    </div>
  </div>
</template>
<style src="./dialog-common.css"></style>

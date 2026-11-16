<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import ModelSelector from '@/components/chat/ModelSelector.vue'
import KnowledgeBaseSelect from '@/components/chat/knowledgeBaseSelect.vue'
import type { KnowledgeSelectorHandle } from '@/composables/useKnowledgeFileDrop'

defineProps<{
  locked: boolean
  modelGroups: NonNullable<InstanceType<typeof ModelSelector>['$props']['modelGroups']>
  isVendorIncomplete: (providerCode: string) => boolean
  roles: { id: string; name: string }[]
  roleId: string
  taskPlan: boolean
  busy: boolean
  modelPopperClass?: string
}>()
const modelId = defineModel<string>('modelId')
const knowledgeId = defineModel<string[]>('knowledgeId', { required: true })
const executionMode = defineModel<'auto' | 'manual' | 'step'>('executionMode', { required: true })
const emit = defineEmits<{ visibleChange: [visible: boolean]; modelChange: [id: string | undefined]; roleChange: [id: unknown] }>()
const { t } = useI18n()
const knowledgeSelectorRef = ref<KnowledgeSelectorHandle | null>(null)
// Keep the original file-drop contract without exposing the component tree or Store.
defineExpose({
  open: () => knowledgeSelectorRef.value?.open(),
  close: () => knowledgeSelectorRef.value?.close(),
})
</script>

<template>
          <div v-if="!locked" class="input-selectors fastclaw-workspace-selectors">
            <div class="selectors-left">
              <ModelSelector v-model="modelId" :model-groups="modelGroups" icon-tags show-incomplete-filter
                :is-vendor-incomplete="isVendorIncomplete" :popper-class="['model-selector-popper main-chat-select-popper', modelPopperClass].filter(Boolean).join(' ')"
                @visible-change="emit('visibleChange', $event)" @change="emit('modelChange', $event)" />
              <KnowledgeBaseSelect ref="knowledgeSelectorRef" v-model="knowledgeId" />
              <el-select :model-value="roleId" size="default"
                style="width:160px" clearable placement="top" :placeholder="t('chat.defaultRole')" @change="emit('roleChange', $event)">
                <el-option value="" :label="t('chat.defaultRole')" />
                <el-option v-for="role in roles" :key="role.id" :value="role.id" :label="role.name" />
              </el-select>
              <el-select v-if="taskPlan" v-model="executionMode"
                size="default" style="width:140px" placement="top" :disabled="busy" placeholder="规划模式">
                <el-option label="自动执行" value="auto" />
                <el-option label="手动确认" value="manual" />
                <el-option label="逐步确认" value="step" />
              </el-select>
            </div>
          </div>
</template>

<style scoped lang="scss">
@use './composerSelectors';
</style>

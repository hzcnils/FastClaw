<script setup lang="ts">
import { onBeforeUnmount, ref, shallowRef, watch, type Component } from 'vue'
import { useI18n } from 'vue-i18n'

defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<{
  loader: () => Promise<Component>
  label: string
  presentation?: 'inline' | 'dialog'
}>(), { presentation: 'inline' })
const emit = defineEmits<{ cancel: [] }>()
const { t } = useI18n()
const panel = shallowRef<Component>()
const loading = ref(false)
const failed = ref(false)
let generation = 0
let disposed = false

async function load() {
  const request = ++generation
  panel.value = undefined
  loading.value = true
  failed.value = false
  try {
    const component = await props.loader()
    if (!disposed && request === generation) panel.value = component
  } catch {
    if (!disposed && request === generation) failed.value = true
  } finally {
    if (!disposed && request === generation) loading.value = false
  }
}
watch(() => props.loader, load, { immediate: true })
onBeforeUnmount(() => { disposed = true; generation++ })
</script>

<template>
  <component v-if="panel" :is="panel" v-bind="$attrs" />
  <el-dialog v-else-if="presentation === 'dialog'" :model-value="true" :title="label"
    width="720px" align-center append-to-body @update:model-value="(visible: boolean) => { if (!visible) emit('cancel') }">
    <div class="chat-panel-placeholder" :role="failed ? 'alert' : 'status'" :aria-busy="loading">
      <span>{{ failed ? t('chat.optionalPanelLoadFailed') : t('common.loading') }}</span>
      <button v-if="failed" type="button" @click="load">{{ t('common.retry') }}</button>
    </div>
  </el-dialog>
  <div v-else class="chat-panel-placeholder" :role="failed ? 'alert' : 'status'" :aria-busy="loading">
    <span>{{ label }} · {{ failed ? t('chat.optionalPanelLoadFailed') : t('common.loading') }}</span>
    <button v-if="failed" type="button" @click="load">{{ t('common.retry') }}</button>
    <button type="button" @click="emit('cancel')">{{ t('common.close') }}</button>
  </div>
</template>

<style scoped>
.chat-panel-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  min-height: 100px;
  padding: 16px;
  color: var(--text-secondary);
}
.chat-panel-placeholder button {
  color: var(--text-primary);
  cursor: pointer;
  background: var(--bg-primary);
  border: 1px solid var(--chat-divider-color);
  border-radius: 6px;
  padding: 4px 10px;
}
</style>

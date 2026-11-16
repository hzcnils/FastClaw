<script setup lang="ts">
defineProps<{ selectedCount: number; allSelected: boolean }>()
const emit = defineEmits<{ selectAll: []; toggle: [enabled: boolean]; delete: [] }>()
</script>

<template>
  <div class="manage-bar">
    <button class="btn-select-all" @click="emit('selectAll')">
      {{ allSelected ? $t('modelManage.deselectAll') : $t('modelManage.selectAll') }}
    </button>
    <span class="selected-count">{{ $t('modelManage.selectedCount', { n: selectedCount }) }}</span>
    <div class="manage-spacer" />
    <button class="btn-start" :disabled="selectedCount === 0" @click="emit('toggle', true)">{{ $t('modelManage.enable') }}</button>
    <button class="btn-stop" :disabled="selectedCount === 0" @click="emit('toggle', false)">{{ $t('modelManage.disable') }}</button>
    <button class="btn-detele" :disabled="selectedCount === 0" @click="emit('delete')">{{ $t('common.delete') }}</button>
  </div>
</template>

<style scoped lang="scss">
@use '../../styles/model-batch-bar';
</style>

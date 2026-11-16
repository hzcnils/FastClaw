<script setup lang="ts">
import ModelCard from '@/components/model/ModelCard.vue'
import ModelRanking from '@/components/model/ModelRanking.vue'
import type { ModelRating, RawModelResponse } from '../../types/contracts'
import { modelRatingKey } from '../../types/contracts'

defineProps<{
  viewMode: 'card' | 'ranking'
  models: RawModelResponse[]
  providerCode: string
  manageMode: boolean
  selectedModelIds: Set<string>
  ratings: Record<string, ModelRating>
}>()

const emit = defineEmits<{
  edit: [model: RawModelResponse]
  delete: [model: RawModelResponse]
  toggle: [model: RawModelResponse]
  select: [id: string]
}>()
</script>

<template>
  <ModelRanking v-if="viewMode === 'ranking'" />
  <div v-else class="model-cards">
    <ModelCard
      v-for="model in models"
      :key="model.id"
      :model="model"
      :provider-code="providerCode"
      :manage-mode="manageMode"
      :is-selected="selectedModelIds.has(model.id)"
      :rating="ratings[modelRatingKey(model.model_id, model.provider_code)] || null"
      @edit="emit('edit', $event)"
      @delete="emit('delete', $event)"
      @toggle="emit('toggle', $event)"
      @select="emit('select', model.id)"
    />
    <div v-if="!models.length" class="empty-model">
      <svg viewBox="0 0 24 24" width="32" height="32" stroke="currentColor" stroke-width="1.5" fill="none">
        <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
        <polyline points="3.27 6.96 12 12.01 20.73 6.96" /><line x1="12" y1="22.08" x2="12" y2="12" />
      </svg>
      <div>{{ $t('modelManage.noModelsHint') }}</div>
    </div>
  </div>
</template>

<style scoped lang="scss">
@use '../../styles/model-catalog-content';
</style>

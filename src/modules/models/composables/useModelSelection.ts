import { computed, ref, watch, type Ref } from 'vue'
import type { RawModelResponse } from '../types/contracts'

export function useModelSelection(models: Ref<RawModelResponse[]>) {
  const search = ref('')
  const manageMode = ref(false)
  const selectedModelIds = ref<Set<string>>(new Set())

  const filteredModels = computed(() => {
    const query = search.value.trim().toLowerCase()
    if (!query) return models.value
    return models.value.filter(model =>
      (model.model_name || '').toLowerCase().includes(query)
      || model.model_id.toLowerCase().includes(query)
      || model.display_name.toLowerCase().includes(query)
      || model.model_label.toLowerCase().includes(query)
      || model.model_type.toLowerCase().includes(query),
    )
  })

  const isAllSelected = computed(() =>
    filteredModels.value.length > 0
    && filteredModels.value.every(model => selectedModelIds.value.has(model.id)),
  )

  watch(models, current => {
    const validIds = new Set(current.map(model => model.id))
    const next = new Set([...selectedModelIds.value].filter(id => validIds.has(id)))
    if (next.size !== selectedModelIds.value.size) selectedModelIds.value = next
  })

  function clearSelection() {
    selectedModelIds.value = new Set()
  }

  function toggleManageMode() {
    manageMode.value = !manageMode.value
    if (!manageMode.value) clearSelection()
  }

  function toggleModelSelection(id: string) {
    const next = new Set(selectedModelIds.value)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    selectedModelIds.value = next
  }

  function toggleSelectAll() {
    selectedModelIds.value = isAllSelected.value
      ? new Set()
      : new Set(filteredModels.value.map(model => model.id))
  }

  return {
    search, manageMode, selectedModelIds, filteredModels, isAllSelected,
    clearSelection, toggleManageMode, toggleModelSelection, toggleSelectAll,
  }
}

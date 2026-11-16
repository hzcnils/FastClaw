import type { Ref } from 'vue'
import type { useModelCatalogStore } from '../stores/modelCatalog.store'

type ModelCatalogStore = ReturnType<typeof useModelCatalogStore>

export function useModelBatchActions(
  catalog: ModelCatalogStore,
  selectedModelIds: Ref<Set<string>>,
  manageMode: Ref<boolean>,
) {
  function resetBatchState() {
    selectedModelIds.value = new Set()
    manageMode.value = false
  }

  async function batchToggleModels(enabled: boolean) {
    if (selectedModelIds.value.size === 0 || !catalog.selectedProvider) return false
    await catalog.patchModels([...selectedModelIds.value], { is_enabled: enabled })
    resetBatchState()
    return true
  }

  async function batchDeleteModels() {
    if (selectedModelIds.value.size === 0 || !catalog.selectedProvider) return false
    await catalog.removeModels([...selectedModelIds.value])
    resetBatchState()
    return true
  }

  return { resetBatchState, batchToggleModels, batchDeleteModels }
}

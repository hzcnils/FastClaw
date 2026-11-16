import { ref, watch, type Ref } from 'vue'
import type { ProviderUpdatePatch } from '../types/contracts'
import type { SelectedModelProvider } from '../stores/modelCatalog.store'

export function useProviderEditor(
  selectedProvider: Ref<SelectedModelProvider | null>,
  updateProviderFields: (providerId: string, patch: ProviderUpdatePatch) => Promise<void>,
) {
  const editingBaseUrl = ref(false)
  const editingBaseUrlValue = ref('')
  const initialBaseUrl = ref('')
  const apiKeyOriginal = ref('')

  function trackInitialBaseUrl() {
    const value = selectedProvider.value?.base_url || ''
    initialBaseUrl.value = value
    editingBaseUrlValue.value = value
  }

  watch(() => selectedProvider.value?.id, trackInitialBaseUrl, { immediate: true })

  function startEditBaseUrl() {
    editingBaseUrlValue.value = selectedProvider.value?.base_url || ''
    editingBaseUrl.value = true
  }

  async function confirmBaseUrl() {
    if (!selectedProvider.value) return false
    await updateProviderFields(selectedProvider.value.id, { base_url: editingBaseUrlValue.value })
    editingBaseUrl.value = false
    return true
  }

  function cancelEditBaseUrl() {
    editingBaseUrlValue.value = selectedProvider.value?.base_url || ''
    editingBaseUrl.value = false
  }

  async function resetBaseUrl() {
    if (!selectedProvider.value) return
    await updateProviderFields(selectedProvider.value.id, { base_url: initialBaseUrl.value })
    editingBaseUrlValue.value = initialBaseUrl.value
  }

  function onApiKeyFocus(currentValue: string | null) {
    apiKeyOriginal.value = currentValue || ''
  }

  async function saveApiKey(newValue: string | null) {
    if (!selectedProvider.value) return 'unchanged' as const
    const nextValue = newValue || ''
    if (nextValue === apiKeyOriginal.value) return 'unchanged' as const
    const original = apiKeyOriginal.value
    try {
      await updateProviderFields(selectedProvider.value.id, { api_key_ref: nextValue })
      return 'saved' as const
    } catch (cause) {
      selectedProvider.value.api_key = original
      throw cause
    }
  }

  return {
    editingBaseUrl, editingBaseUrlValue, initialBaseUrl,
    trackInitialBaseUrl, startEditBaseUrl, confirmBaseUrl, cancelEditBaseUrl,
    resetBaseUrl, onApiKeyFocus, saveApiKey,
  }
}

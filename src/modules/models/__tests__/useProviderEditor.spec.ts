import { describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import { useProviderEditor } from '../composables/useProviderEditor'
import type { SelectedModelProvider } from '../stores/modelCatalog.store'

const provider: SelectedModelProvider = {
  id: 'provider-1', name: 'Provider', provider_type: 'openai', base_url: 'https://old',
  api_key: 'old-key', is_local: false, is_enabled: true, provider_code: 'OPENAI',
  source_category: 'online', extra_config: null,
}

describe('useProviderEditor', () => {
  it('skips an unchanged key and saves a changed full key', async () => {
    const selected = ref<SelectedModelProvider | null>({ ...provider })
    const update = vi.fn(async (_id: string, patch: { api_key_ref?: string | null }) => {
      if (patch.api_key_ref !== undefined && selected.value) selected.value.api_key = patch.api_key_ref
    })
    const editor = useProviderEditor(selected, update)

    editor.onApiKeyFocus('old-key')
    await expect(editor.saveApiKey('old-key')).resolves.toBe('unchanged')
    await expect(editor.saveApiKey('new-key')).resolves.toBe('saved')
    expect(update).toHaveBeenCalledWith('provider-1', { api_key_ref: 'new-key' })
  })

  it('restores the original key when saving fails', async () => {
    const selected = ref<SelectedModelProvider | null>({ ...provider })
    const editor = useProviderEditor(selected, async () => { throw new Error('failed') })
    editor.onApiKeyFocus('old-key')
    if (selected.value) selected.value.api_key = 'new-key'

    await expect(editor.saveApiKey('new-key')).rejects.toThrow('failed')
    expect(selected.value?.api_key).toBe('old-key')
  })

  it('tracks, cancels and resets the current base URL', async () => {
    const selected = ref<SelectedModelProvider | null>({ ...provider })
    const update = vi.fn(async (_id: string, patch: { base_url?: string }) => {
      if (patch.base_url !== undefined && selected.value) selected.value.base_url = patch.base_url
    })
    const editor = useProviderEditor(selected, update)
    editor.startEditBaseUrl()
    editor.editingBaseUrlValue.value = 'https://changed'
    editor.cancelEditBaseUrl()
    expect(editor.editingBaseUrlValue.value).toBe('https://old')

    await editor.resetBaseUrl()
    expect(update).toHaveBeenCalledWith('provider-1', { base_url: 'https://old' })
  })
})

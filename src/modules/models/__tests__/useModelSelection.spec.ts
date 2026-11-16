import { describe, expect, it } from 'vitest'
import { ref } from 'vue'
import { useModelSelection } from '../composables/useModelSelection'
import type { RawModelResponse } from '../types/contracts'

const models: RawModelResponse[] = [
  { id: 'one', model_id: 'gpt-4o', display_name: 'GPT-4o', model_type: 'chat', is_enabled: true, model_label: '视觉', mxcs: null, context_window: null, provider_code: 'OPENAI', supports_vision: true },
  { id: 'two', model_id: 'embed-small', display_name: 'Embedding', model_type: 'embedding', is_enabled: true, model_label: '', mxcs: null, context_window: null, provider_code: 'OPENAI', supports_vision: false },
]

describe('useModelSelection', () => {
  it('filters across model fields and selects the visible result set', () => {
    const source = ref(models)
    const selection = useModelSelection(source)
    selection.search.value = '视觉'
    expect(selection.filteredModels.value.map(model => model.id)).toEqual(['one'])

    selection.toggleSelectAll()
    expect([...selection.selectedModelIds.value]).toEqual(['one'])
    expect(selection.isAllSelected.value).toBe(true)
  })

  it('removes selections that no longer exist after refresh', async () => {
    const source = ref(models)
    const selection = useModelSelection(source)
    selection.toggleModelSelection('one')
    source.value = [models[1]]
    await Promise.resolve()
    expect(selection.selectedModelIds.value.size).toBe(0)
  })

  it('clears selection when leaving manage mode', () => {
    const selection = useModelSelection(ref(models))
    selection.toggleManageMode()
    selection.toggleModelSelection('one')
    selection.toggleManageMode()
    expect(selection.manageMode.value).toBe(false)
    expect(selection.selectedModelIds.value.size).toBe(0)
  })
})

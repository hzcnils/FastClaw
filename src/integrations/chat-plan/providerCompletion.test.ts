import { flushPromises } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { createProviderCompletion } from './providerCompletion'

const provider = { id: 'provider-a', name: 'Provider A', baseUrl: 'https://example.com/v1', apiKey: '' }
const values = { baseUrl: ' https://example.com/v1 ', apiKey: ' new-key ' }

describe('provider completion send gate', () => {
  it('does not start a provider lookup for an already-aborted preparation', async () => {
    const load = vi.fn(async () => provider), gate = createProviderCompletion({ load, save: vi.fn() })
    const controller = new AbortController(); controller.abort()
    expect(await gate.ensureReady('model', undefined, controller.signal)).toBe(false)
    expect(load).not.toHaveBeenCalled()
  })
  it('cancels its visible form but does not cancel a newer form on repeated abort', async () => {
    const gate = createProviderCompletion({ load: async () => provider, save: vi.fn() })
    const controller = new AbortController(), old = gate.ensureReady('old', undefined, controller.signal)
    await flushPromises(); controller.abort(); expect(await old).toBe(false)
    const next = gate.ensureReady('new'); await flushPromises(); controller.abort()
    expect(gate.pending.value?.modelId).toBe('new'); gate.cancel(); expect(await next).toBe(false)
  })
  it('a completed signal cannot cancel another request', async () => {
    const gate = createProviderCompletion({ load: async () => provider, save: vi.fn() })
    const controller = new AbortController(), old = gate.ensureReady('old', undefined, controller.signal)
    await flushPromises(); await gate.submit(values); expect(await old).toBe(true)
    const next = gate.ensureReady('new'); await flushPromises(); controller.abort()
    expect(gate.pending.value?.modelId).toBe('new'); gate.cancel(); await next
  })
  it('allows global config save to finish after cancellation without resuming the old send', async () => {
    let finish!: () => void
    const gate = createProviderCompletion({ load: async () => provider, save: () => new Promise<void>(done => { finish = done }) })
    const controller = new AbortController(), old = gate.ensureReady('old', undefined, controller.signal)
    await flushPromises(); const saving = gate.submit(values); controller.abort(); expect(await old).toBe(false)
    const next = gate.ensureReady('new'); await flushPromises(); finish(); await saving
    expect(gate.pending.value?.modelId).toBe('new'); gate.cancel(); await next
  })
  it.each(['chat', 'planning'])('continues the original %s request once, only after saving', async mode => {
    const save = vi.fn(async () => {})
    const gate = createProviderCompletion({ load: async () => ({ ...provider }), save })
    const send = vi.fn()
    const request = { mode, modelId: 'model-a', text: 'quoted question', attachments: ['file-a'] }
    const operation = (async () => { if (await gate.ensureReady(request.modelId)) send(request) })()
    await flushPromises()
    expect(send).not.toHaveBeenCalled()
    expect(gate.pending.value?.modelId).toBe('model-a')
    await expect(gate.ensureReady('model-a')).resolves.toBe(false)
    await Promise.all([gate.submit(values), gate.submit(values)])
    await operation
    expect(save).toHaveBeenCalledExactlyOnceWith(provider, { baseUrl: 'https://example.com/v1', apiKey: 'new-key' })
    expect(send).toHaveBeenCalledExactlyOnceWith(request)
    expect(gate.pending.value).toBeNull()
  })

  it('leaves failures editable and resumes only after a successful retry', async () => {
    const save = vi.fn().mockRejectedValueOnce(new Error('database unavailable')).mockResolvedValueOnce(undefined)
    const gate = createProviderCompletion({ load: async () => provider, save })
    const result = gate.ensureReady('model-a')
    await flushPromises()
    await gate.submit(values)
    expect(gate.error.value).toBe('database unavailable')
    expect(gate.pending.value).not.toBeNull()
    await gate.submit(values)
    await expect(result).resolves.toBe(true)
  })

  it('cancels without sending even when saving finishes after switching conversations', async () => {
    let saved!: () => void
    const gate = createProviderCompletion({ load: async () => provider, save: () => new Promise(resolve => { saved = resolve }) })
    const result = gate.ensureReady('model-a')
    await flushPromises()
    const saving = gate.submit(values)
    gate.cancel()
    saved()
    await saving
    await expect(result).resolves.toBe(false)
    expect(gate.pending.value).toBeNull()
  })

  it('does not open an obsolete form when loading completes after cancellation', async () => {
    let loaded!: (value: typeof provider) => void
    const gate = createProviderCompletion({ load: () => new Promise(resolve => { loaded = resolve }), save: vi.fn() })
    const result = gate.ensureReady('model-a')
    gate.cancel()
    loaded(provider)
    await expect(result).resolves.toBe(false)
    expect(gate.pending.value).toBeNull()
  })

  it('passes configured and exempt providers, and propagates read errors', async () => {
    const load = vi.fn().mockResolvedValueOnce({ ...provider, apiKey: 'key' }).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error('read failed'))
    const gate = createProviderCompletion({ load, save: vi.fn() })
    await expect(gate.ensureReady('configured')).resolves.toBe(true)
    await expect(gate.ensureReady('huizhi')).resolves.toBe(true)
    await expect(gate.ensureReady('missing')).rejects.toThrow('read failed')
    expect(gate.pending.value).toBeNull()
  })
})

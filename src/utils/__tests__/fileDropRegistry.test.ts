import { describe, expect, it, vi } from 'vitest'
import { createFileDropRegistry } from '../fileDropRegistry'

describe('file drop routing', () => {
  it('delivers a mixed batch once to the active path only', async () => {
    const registry = createFileDropRegistry()
    const chat = vi.fn(async () => {})
    const document = vi.fn(async (_paths: string[], _isCurrent: () => boolean) => {})
    registry.register({ path: '/chat', enabled: () => true, session: () => 1, receive: chat })
    registry.register({ path: '/documentGeneration', enabled: () => true, session: () => 2, receive: document })
    await registry.dispatch('/documentGeneration', ['a.txt', 'b.txt', 'folder1', 'folder2'])
    expect(chat).not.toHaveBeenCalled()
    expect(document).toHaveBeenCalledTimes(1)
    expect(document.mock.calls[0][0]).toEqual(['a.txt', 'b.txt', 'folder1', 'folder2'])
    expect(registry.accepts('/settings')).toBe(false)
  })

  it('invalidates pending work after session change or unregistration', async () => {
    const registry = createFileDropRegistry()
    let session = 1
    let isCurrent = () => false
    const unregister = registry.register({ path: '/chat', enabled: () => true, session: () => session,
      receive: async (_, current) => { isCurrent = current },
    })
    await registry.dispatch('/chat', ['a.txt'])
    expect(isCurrent()).toBe(true)
    session++
    expect(isCurrent()).toBe(false)
    await registry.dispatch('/chat', ['b.txt'])
    unregister()
    expect(isCurrent()).toBe(false)
    expect(registry.accepts('/chat')).toBe(false)
  })

  it('does not bypass a disabled target', async () => {
    const registry = createFileDropRegistry()
    const receive = vi.fn(async () => {})
    registry.register({ path: '/chat', enabled: () => false, session: () => 1, receive })
    await registry.dispatch('/chat', ['a.txt'])
    expect(receive).not.toHaveBeenCalled()
  })
})

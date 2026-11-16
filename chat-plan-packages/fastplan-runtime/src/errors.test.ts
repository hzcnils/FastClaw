import { describe, expect, it } from 'vitest'
import { fastPlanErrorMessage, normalizeFastPlanError } from './errors'

describe('FastPlan error normalization', () => {
  it('reads the error field from a node_failed protocol event', () => {
    const event = { type: 'node_failed', tree_id: 'tree-1', node_id: 'task-4', error: 'Agent confidence too low: 30' }
    const normalized = normalizeFastPlanError(event, 'FastPlan 节点执行失败')

    expect(normalized.message).toBe('Agent confidence too low: 30')
    expect(normalized.details).toBe(event)
    expect(normalized.message).not.toBe('[object Object]')
  })

  it('unwraps nested object messages and rejects object string coercion', () => {
    expect(fastPlanErrorMessage({ message: { error: { detail: 'model service unavailable' } } })).toBe('model service unavailable')
    expect(fastPlanErrorMessage({ message: '[object Object]' }, 'fallback')).toBe('fallback')
  })

  it('preserves protocol metadata used by host applications', () => {
    const normalized = normalizeFastPlanError({ message: 'authentication failed', code: 401, terminal: true, recoverable: false })
    expect(normalized).toMatchObject({ message: 'authentication failed', code: '401', terminal: true, recoverable: false })
  })
})

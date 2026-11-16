import { describe, expect, it, vi } from 'vitest'
import { CHAT_PLAN_PACKAGES_FLAG, isChatPlanPackagesEnabled, setChatPlanPackagesEnabled } from './featureFlags'

describe('chat plan package feature flag', () => {
  it('defaults to enabled and supports explicit rollback', () => {
    expect(isChatPlanPackagesEnabled({ getItem: () => null })).toBe(true)
    expect(isChatPlanPackagesEnabled({ getItem: () => 'false' })).toBe(false)
  })
  it('writes the stable storage key', () => {
    const setItem = vi.fn()
    setChatPlanPackagesEnabled(false, { setItem })
    expect(setItem).toHaveBeenCalledWith(CHAT_PLAN_PACKAGES_FLAG, 'false')
  })
})


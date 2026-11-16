import { describe, expect, it } from 'vitest'
import { isDownloadTaskActive, isFastKgDownloadTaskActive } from './taskState'

describe('download task state', () => {
  it.each(['pending', 'downloading', 'pausing', 'paused', 'verifying', 'installing'] as const)(
    'treats %s as active',
    (status) => expect(isDownloadTaskActive({ status })).toBe(true),
  )

  it.each(['success', 'failed', 'cancelled'] as const)(
    'treats terminal status %s as inactive',
    (status) => expect(isDownloadTaskActive({ status })).toBe(false),
  )

  it('does not treat an old successful FastKG record as an active download', () => {
    expect(isFastKgDownloadTaskActive({
      resourceId: 'fastkg',
      name: 'FastKG',
      status: 'success',
    })).toBe(false)
  })

  it('recognizes an actual FastKG download in progress', () => {
    expect(isFastKgDownloadTaskActive({
      resourceId: 'fastkg',
      name: 'FastKG',
      status: 'downloading',
    })).toBe(true)
  })
})

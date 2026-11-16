import { beforeEach, describe, expect, it, vi } from 'vitest'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { resolveFastKgDownloadSource } from './fastKgDownloadConnectivity'

const options = {
  internetCheckUrl: 'https://internet.example/favicon.ico',
  publicBaseUrl: 'https://download.example/',
  privateBaseUrl: 'http://192.168.1.10:7000/',
}

describe('resolveFastKgDownloadSource', () => {
  beforeEach(() => {
    invoke.mockReset()
  })

  it('uses the public download address when internet connectivity succeeds', async () => {
    invoke.mockImplementation((_command: string, args: { url: string }) =>
      Promise.resolve(args.url === options.internetCheckUrl),
    )

    const result = await resolveFastKgDownloadSource(options)

    expect(result.source).toEqual({ type: 'public', baseUrl: 'https://download.example' })
    expect(invoke).toHaveBeenCalledWith('check_internet_connectivity', {
      url: 'http://192.168.1.10:7000/api/v1/apps/FastKG',
    })
  })

  it('falls back to the configured private address when internet is unavailable', async () => {
    invoke.mockImplementation((_command: string, args: { url: string }) =>
      Promise.resolve(args.url.includes('192.168.1.10')),
    )

    const result = await resolveFastKgDownloadSource(options)

    expect(result).toEqual({
      internetReachable: false,
      privateReachable: true,
      source: { type: 'private', baseUrl: 'http://192.168.1.10:7000' },
    })
  })

  it('returns no source when neither route is reachable', async () => {
    invoke.mockResolvedValue(false)

    const result = await resolveFastKgDownloadSource(options)

    expect(result.source).toBeNull()
    expect(result.internetReachable).toBe(false)
    expect(result.privateReachable).toBe(false)
  })

  it('keeps public priority when both routes are reachable', async () => {
    invoke.mockResolvedValue(true)

    const result = await resolveFastKgDownloadSource(options)

    expect(result.source).toEqual({ type: 'public', baseUrl: 'https://download.example' })
  })
})

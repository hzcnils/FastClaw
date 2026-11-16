import { describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), write: vi.fn(), download: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('@tauri-apps/api/path', () => ({ tempDir: async () => 'temp/' }))
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile: mocks.write }))
vi.mock('@/stores/appConfig', () => ({ useAppConfigStore: () => ({ config: { remote_servers: { download_api_base_url: 'test' } } }) }))
vi.mock('@/integrations/download-center', () => ({ useFastClawDownloadDriver: () => ({ startFile2MdDownload: mocks.download }) }))
import { recognizeImageText } from './imageOcr'

describe('OCR host resources', () => {
  it('shares installation across tabs; aborting one attachment leaves the other OCR intact', async () => {
    let complete!: (task: { status: string }) => void
    mocks.download.mockImplementation(() => new Promise(resolve => { complete = resolve }))
    mocks.invoke.mockImplementation(async (command: string) => command === 'check_file2md_exists' ? false : command === 'file2md_ocr_by_path' ? 'text' : undefined)
    const file = { type: 'image/png', arrayBuffer: async () => new ArrayBuffer(1) } as File
    const a = new AbortController(), b = new AbortController()
    const first = recognizeImageText(file, '', a.signal)
    const rejected = expect(first).rejects.toMatchObject({ name: 'AbortError' })
    const second = recognizeImageText(file, '', b.signal)
    await flushPromises()
    expect(mocks.download).toHaveBeenCalledTimes(1)
    a.abort()
    await rejected
    complete({ status: 'success' })
    await expect(second).resolves.toBe('text')
    expect(mocks.write).toHaveBeenCalledTimes(1)
    expect(mocks.invoke.mock.calls.filter(([name]) => name === 'file2md_cleanup_temp')).toHaveLength(1)
  })

  it('handles a silent failed download without starting OCR', async () => {
    mocks.invoke.mockClear()
    mocks.download.mockResolvedValue({ status: 'failed' })
    await expect(recognizeImageText(undefined, 'data:x', new AbortController().signal)).rejects.toThrow('OCR插件下载失败')
    expect(mocks.invoke.mock.calls.some(([name]) => name === 'file2md_ocr_by_path')).toBe(false)
  })
})

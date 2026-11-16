// @vitest-environment jsdom
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { save } from '@tauri-apps/plugin-dialog'
import { writeFile, writeTextFile } from '@tauri-apps/plugin-fs'
import { invoke } from '@tauri-apps/api/core'
import { Packer } from 'docx'
import html2canvas from 'html2canvas'
import { exportMarkdown, exportPdf, exportWord } from '@/utils/exportMessage'

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn() }))
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile: vi.fn(), writeTextFile: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('html2canvas', () => ({ default: vi.fn() }))
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason?: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
beforeEach(() => {
  vi.mocked(save).mockReset().mockResolvedValue('chosen-file')
  vi.mocked(writeFile).mockReset().mockResolvedValue(); vi.mocked(writeTextFile).mockReset().mockResolvedValue()
  vi.mocked(invoke).mockReset().mockResolvedValue([1, 2, 3]); vi.mocked(html2canvas).mockReset()
})
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); document.body.innerHTML = '' })
describe('real export utility write boundaries', () => {
  it('retains unguarded legacy Markdown behavior and cancellation', async () => {
    expect(await exportMarkdown('snapshot')).toBe(true); expect(writeTextFile).toHaveBeenCalledWith('chosen-file', 'snapshot')
    vi.mocked(save).mockResolvedValue(null); expect(await exportMarkdown('canceled')).toBe(false)
    expect(writeTextFile).toHaveBeenCalledOnce()
  })
  it('rejects an invalid Markdown request before showing a dialog', async () => {
    await expect(exportMarkdown('text', { isCurrent: () => false })).rejects.toMatchObject({ name: 'AbortError' })
    expect(save).not.toHaveBeenCalled()
  })
  it('does not write Markdown after an old native dialog returns a filename', async () => {
    const dialog = deferred<string>(), state = { current: true }; vi.mocked(save).mockReturnValue(dialog.promise)
    const task = exportMarkdown('text', { isCurrent: () => state.current }); state.current = false; dialog.resolve('old')
    await expect(task).rejects.toMatchObject({ name: 'AbortError' }); expect(writeTextFile).not.toHaveBeenCalled()
  })
  it('allows an already-started write to finish on its captured path', async () => {
    const write = deferred<void>(), state = { current: true }; vi.mocked(writeTextFile).mockReturnValue(write.promise)
    const task = exportMarkdown('snapshot', { isCurrent: () => state.current }); await flushPromises()
    state.current = false; write.resolve(); expect(await task).toBe(true)
    expect(writeTextFile).toHaveBeenCalledExactlyOnceWith('chosen-file', 'snapshot')
  })
  it('does not show a PDF save dialog after stale backend generation completes', async () => {
    const generation = deferred<number[]>(), state = { current: true }; vi.mocked(invoke).mockReturnValue(generation.promise)
    const task = exportPdf('pdf snapshot', x => x, undefined, { isCurrent: () => state.current }); await flushPromises()
    expect(invoke).toHaveBeenCalledWith('export_pdf', { markdown: 'pdf snapshot' })
    state.current = false; generation.resolve([1]); expect(await task).toMatchObject({ canceled: true })
    expect(save).not.toHaveBeenCalled(); expect(writeFile).not.toHaveBeenCalled()
  })
  it('does not write a PDF chosen after the request expires', async () => {
    const dialog = deferred<string>(), state = { current: true }; vi.mocked(save).mockReturnValue(dialog.promise)
    const task = exportPdf('pdf', x => x, undefined, { isCurrent: () => state.current }); await flushPromises()
    state.current = false; dialog.resolve('old'); expect(await task).toMatchObject({ canceled: true })
    expect(writeFile).not.toHaveBeenCalled()
  })
  it('preserves PDF success and real failure results', async () => {
    expect(await exportPdf('pdf', x => x)).toMatchObject({ success: true, filePath: 'chosen-file' })
    expect(writeFile).toHaveBeenCalledWith('chosen-file', new Uint8Array([1, 2, 3]))
    vi.mocked(invoke).mockRejectedValue(new Error('generation failed'))
    expect(await exportPdf('pdf', x => x)).toEqual({ success: false, error: 'generation failed' })
  })
  it('does not show Word save dialog after stale packing completes', async () => {
    const packing = deferred<Blob>(), state = { current: true }; vi.spyOn(Packer, 'toBlob').mockReturnValue(packing.promise)
    const task = exportWord('doc', () => '<p>doc</p>', { isCurrent: () => state.current }); await flushPromises()
    state.current = false; packing.resolve({} as Blob)
    await expect(task).rejects.toMatchObject({ name: 'AbortError' }); expect(save).not.toHaveBeenCalled()
  })
  it('checks again after Word blob byte conversion, before writing', async () => {
    const bytes = deferred<ArrayBuffer>(), state = { current: true }
    vi.spyOn(Packer, 'toBlob').mockResolvedValue({ arrayBuffer: () => bytes.promise } as Blob)
    const task = exportWord('doc', () => '<p>doc</p>', { isCurrent: () => state.current }); await flushPromises()
    expect(save).toHaveBeenCalledOnce(); state.current = false; bytes.resolve(new ArrayBuffer(1))
    await expect(task).rejects.toMatchObject({ name: 'AbortError' }); expect(writeFile).not.toHaveBeenCalled()
  })
  it('removes temporary formula DOM when the export expires during conversion', async () => {
    const conversion = deferred<HTMLCanvasElement>(), state = { current: true }; vi.mocked(html2canvas).mockReturnValue(conversion.promise)
    const task = exportWord('formula', () => '<span class="katex">x</span>', { isCurrent: () => state.current })
    await flushPromises(); expect(document.querySelector('.katex')).not.toBeNull()
    state.current = false; conversion.resolve({ toDataURL: () => 'data:image/png;base64,a' } as HTMLCanvasElement)
    await expect(task).rejects.toMatchObject({ name: 'AbortError' })
    expect(document.querySelector('.katex')).toBeNull(); expect(save).not.toHaveBeenCalled()
  })
  it('settles failed SVG rasterization and releases its URL and temporary DOM', async () => {
    const revoke = vi.fn()
    vi.stubGlobal('URL', { createObjectURL: () => 'blob:formula', revokeObjectURL: revoke })
    vi.stubGlobal('Image', class { onload?: () => void; set src(_value: string) { this.onload?.() } })
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => { throw new Error('canvas') })
    vi.spyOn(Packer, 'toBlob').mockResolvedValue({ arrayBuffer: async () => new ArrayBuffer(1) } as Blob)
    expect(await exportWord('formula', () => '<span class="katex"><svg viewBox="0 0 10 10"></svg></span>')).toBe(true)
    expect(revoke).toHaveBeenCalledExactlyOnceWith('blob:formula'); expect(document.querySelector('.katex')).toBeNull()
    expect(writeFile).toHaveBeenCalledOnce()
  })
})

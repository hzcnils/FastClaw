import type { ChatAttachment } from '@/types'
import type { ChatTabState } from './tabState'

interface UploadResult { success?: boolean; data?: { knowledge_base_id?: string; kb_id?: string; file_id?: string }; message?: string; error?: string }
export interface UploadDependencies {
  upload(file: File, signal: AbortSignal): Promise<UploadResult>
  persist(files: ChatAttachment[]): void
}

/** No component, route or DOM references: late results only commit to their owner. */
export async function uploadTabDocument(owner: ChatTabState, file: File, deps: UploadDependencies) {
  if (owner.disposed) return
  const id = crypto.randomUUID()
  const { fileList, fileKbIds, conversationFileIds, pendingAutoSend } = owner.state
  fileList.value.push({ id, name: file.name, type: 'document', size: file.size, status: 'uploading' })
  owner.retainFile(id, file)
  const operation = owner.beginAttachment(id)
  try {
    const result = await deps.upload(file, operation.signal)
    if (!operation.current()) return
    if (!result.success || !result.data) throw new Error(result.message || result.error || '上传失败，请重试')
    const kbId = result.data.knowledge_base_id || result.data.kb_id
    const fileId = result.data.file_id
    const index = fileList.value.findIndex(f => f.id === id)
    fileList.value[index] = { ...fileList.value[index], status: 'uploaded', uploaded_kb_id: kbId, uploaded_file_id: fileId }
    if (kbId && !fileKbIds.value.includes(kbId)) fileKbIds.value.push(kbId)
    if (fileId && !conversationFileIds.value.includes(fileId)) conversationFileIds.value.push(fileId)
    deps.persist(fileList.value.filter(f => f.type === 'document' && f.status === 'uploaded'))
  } catch (error) {
    if (!operation.current()) return
    const index = fileList.value.findIndex(f => f.id === id)
    fileList.value[index] = { ...fileList.value[index], status: 'error', error_message: error instanceof Error ? error.message : String(error) }
    pendingAutoSend.value = false
  } finally { operation.finish() }
}

export async function recognizeTabImage(owner: ChatTabState, id: string, recognize: (file: File | undefined, previewUrl: string, signal: AbortSignal) => Promise<string>) {
  const file = owner.state.fileList.value.find(f => f.id === id)
  if (owner.disposed || !file) return
  const previewUrl = file.preview_url || ''
  const rawFile = owner.files.get(id)
  const operation = owner.beginAttachment(id)
  const update = (patch: Partial<ChatAttachment>) => {
    if (!operation.current()) return
    const index = owner.state.fileList.value.findIndex(f => f.id === id)
    owner.state.fileList.value[index] = { ...owner.state.fileList.value[index], ...patch }
  }
  update({ status: 'uploading', error_message: undefined })
  try {
    const text = await recognize(rawFile, previewUrl, operation.signal)
    update({ status: 'pending', ocr_text: text.trim(), error_message: undefined })
  } catch (error) {
    update({ status: 'error', error_message: `OCR识别失败：${error instanceof Error ? error.message : String(error)}` })
    if (operation.current()) owner.state.pendingAutoSend.value = false
  } finally { operation.finish() }
}

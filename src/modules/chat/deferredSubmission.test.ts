// @vitest-environment jsdom
import { createPinia, setActivePinia } from 'pinia'
import { effectScope, nextTick, reactive } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createChatTabState, type ChatTabState } from './tabState'
import { uploadTabDocument } from './attachmentTasks'
import { DeferredChatSubmissionService, type DeferredChatSubmission } from './deferredSubmission'
import { useExecutionActivityStore } from '@/stores/executionActivity'

function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes }); return { promise, resolve } }
const owners: ChatTabState[] = []
let sequence = 0
function setup() {
  const owner = createChatTabState(`deferred-${++sequence}`)
  owners.push(owner)
  const state = owner.state
  state.messageStr.value = '原问题'; state.quotedText.value = '引用'; state.modelId.value = 'model'
  state.dialogueCharacter.value.taskPlan = false
  state.fileList.value = [{ id: 'doc', name: 'file.txt', type: 'document', status: 'uploading' }]
  const store = reactive({ currentConversationId: 'source', sendMessage: vi.fn(async (..._args: any[]) => true), setConvFileCache: vi.fn() })
  const input: DeferredChatSubmission = {
    owner, store, tabPath: `/chat/${owner.instanceId}`, conversationId: 'source',
    raw: '原问题', quote: '引用', content: '引用\n原问题', modelId: 'model', vision: false,
    knowledgeBaseIds: ['selected-kb'], localKbPaths: [{ path: 'C:/docs', recursive: true }],
    webSearch: true, searchProvider: 'search', thinking: true, mcpTools: true, mcpServerIds: ['tool'],
    systemPrompt: '角色', assistantId: 'agent', assistantDisplay: { agent_name: 'Agent' },
    skillName: 'skill', enabledSkills: ['weather'], providerCode: 'provider',
  }
  const complete = () => {
    state.fileList.value = [{ ...state.fileList.value[0], status: 'uploaded', uploaded_kb_id: 'upload-kb', uploaded_file_id: 'file-id' }]
    state.fileKbIds.value.push('upload-kb'); state.conversationFileIds.value.push('file-id')
  }
  return { owner, state, store, input, complete }
}

describe('accepted upload-wait commands', () => {
  beforeEach(() => setActivePinia(createPinia()))
  afterEach(() => { owners.splice(0).forEach(owner => owner.release()) })

  it('prepares an immediate image send after view eviction using only accepted options', async () => {
    const c = setup(), gate = deferred<string>(), reader = vi.fn(() => gate.promise)
    const service = new DeferredChatSubmissionService(reader), scope = effectScope()
    c.input.waitForUploads = false; c.input.vision = true
    c.state.fileList.value = [{ id: 'image', name: 'image.png', type: 'image', status: 'pending', preview_url: 'blob:image' }]
    const task = scope.run(() => service.submit(c.input))!
    scope.stop()
    await vi.waitFor(() => expect(reader).toHaveBeenCalledTimes(1))
    c.input.mcpServerIds.push('later-tool'); c.input.localKbPaths[0].path = 'later'
    c.input.thinking = false; c.input.systemPrompt = 'later-role'
    expect(c.state.messageStr.value).toBe('原问题')
    expect(c.state.pendingAutoSend.value).toBe(true)
    expect(service.submit(c.input)).toBe(task)
    gate.resolve('data:image/png;base64,A')
    expect(await task).toBe(true)
    const args = c.store.sendMessage.mock.calls[0]
    expect(c.store.sendMessage).toHaveBeenCalledTimes(1)
    expect(args[5]).toBe('角色')
    expect(args[8]).toEqual([{ path: 'C:/docs', recursive: true }])
    expect(args[12]).toEqual(['data:image/png;base64,A'])
    expect(args[13]).toBe(true)
    expect(args[16]).toEqual(['tool'])
  })

  it.each(['stop', 'close', 'context', 'draft'] as const)('cancels immediate image conversion on %s', async reason => {
    const c = setup(), gate = deferred<string>(), reader = vi.fn(() => gate.promise)
    const service = new DeferredChatSubmissionService(reader)
    c.input.waitForUploads = false
    c.state.fileList.value = [{ id: 'image', name: 'image.png', type: 'image', status: 'pending', preview_url: 'blob:image' }]
    const task = service.submit(c.input)
    await vi.waitFor(() => expect(reader).toHaveBeenCalledTimes(1))
    if (reason === 'stop') service.cancel(c.owner)
    if (reason === 'close') c.owner.release()
    if (reason === 'context') c.owner.invalidateContext()
    if (reason === 'draft') c.state.messageStr.value = '新的草稿'
    expect(await task).toBe(false)
    gate.resolve('data:image/png;base64,LATE'); await nextTick()
    expect(c.store.sendMessage).not.toHaveBeenCalled()
    expect(c.state.chatSubmissionError.value).toBe('')
    if (reason === 'draft') expect(c.state.messageStr.value).toBe('新的草稿')
  })

  it('does not accept an unconfirmed upload in immediate mode', async () => {
    const c = setup(), service = new DeferredChatSubmissionService()
    c.input.waitForUploads = false
    expect(await service.submit(c.input)).toBe(false)
    c.complete(); await nextTick()
    expect(c.store.sendMessage).not.toHaveBeenCalled()
    expect(c.state.chatSubmissionError.value).toContain('确认等待')
    expect(c.state.messageStr.value).toBe('原问题')
  })

  it('preserves previous document cache and file context on a plain text send', async () => {
    const c = setup(), service = new DeferredChatSubmissionService()
    c.input.waitForUploads = false; c.state.fileList.value = []
    c.state.fileKbIds.value = ['previous-kb']; c.state.conversationFileIds.value = ['previous-file']
    expect(await service.submit(c.input)).toBe(true)
    expect(c.store.setConvFileCache).not.toHaveBeenCalled()
    expect(c.store.sendMessage.mock.calls[0][2]).toEqual(['selected-kb', 'previous-kb'])
    expect(c.store.sendMessage.mock.calls[0][9]).toEqual(['previous-file'])
  })

  it('numbers OCR using the original image order and does not mutate draft previews on failure', async () => {
    const c = setup(), service = new DeferredChatSubmissionService(async () => 'data:image/png;base64,A')
    c.input.waitForUploads = false
    c.state.fileList.value = [
      { id: 'first', name: '1.png', type: 'image', status: 'pending', preview_url: 'blob:1' },
      { id: 'second', name: '2.png', type: 'image', status: 'pending', preview_url: 'blob:2', ocr_text: '第二张' },
    ]
    expect(await service.submit(c.input)).toBe(true)
    expect(c.store.sendMessage.mock.calls[0][17]).toContain('[图片2 OCR结果]')
    c.state.messageStr.value = '重试'; c.state.fileList.value = [{ id: 'first', name: '1.png', type: 'image', status: 'pending', preview_url: 'blob:1' }]
    const failing = new DeferredChatSubmissionService(async () => { throw new Error('读取失败') })
    expect(await failing.submit(c.input)).toBe(false)
    expect(c.state.fileList.value[0].preview_url).toBe('blob:1')
    expect(c.store.sendMessage).toHaveBeenCalledTimes(1)
  })

  it('survives calling view scope disposal, deduplicates and uses an accepted parameter snapshot', async () => {
    const c = setup(), service = new DeferredChatSubmissionService(), scope = effectScope()
    c.state.fileKbIds.value = ['previous-kb']; c.state.conversationFileIds.value = ['previous-file']
    const task = scope.run(() => service.submit(c.input))!
    expect(service.submit(c.input)).toBe(task)
    expect(useExecutionActivityStore().isTabProtected(c.input.tabPath)).toBe(true)
    scope.stop()
    c.input.knowledgeBaseIds.push('later'); c.input.localKbPaths[0].path = 'D:/new'
    c.input.mcpServerIds.push('later-tool'); c.input.assistantDisplay!.agent_name = 'later-agent'
    c.complete()
    expect(await task).toBe(true)
    expect(c.store.sendMessage).toHaveBeenCalledTimes(1)
    const args = c.store.sendMessage.mock.calls[0]
    expect(args.slice(0, 3)).toEqual(['引用\n原问题', 'model', ['selected-kb', 'previous-kb', 'upload-kb']])
    expect(args[8]).toEqual([{ path: 'C:/docs', recursive: true }])
    expect(args[9]).toEqual(['previous-file', 'file-id'])
    expect(args.slice(10, 17)).toEqual([true, 'search', [], true, { agent_name: 'Agent' }, true, ['tool']])
    // 尾部三个位置参数：skillName / enabledSkills / providerCode。
    // 少断言一个就会让「白名单没送到后端」这类错位静默通过。
    expect(args.slice(18, 21)).toEqual(['skill', ['weather'], 'provider'])
    expect(c.state.messageStr.value).toBe('')
    expect(c.state.fileList.value).toEqual([])
    expect(c.state.pendingAutoSend.value).toBe(false)
    expect(c.store.setConvFileCache).toHaveBeenCalledWith('source', [expect.objectContaining({ uploaded_file_id: 'file-id' })])
    expect(useExecutionActivityStore().isTabProtected(c.input.tabPath)).toBe(false)
  })

  it('keeps two background tabs isolated', async () => {
    const a = setup(), b = setup(), service = new DeferredChatSubmissionService()
    b.store.currentConversationId = b.input.conversationId = 'other'
    b.input.content = '另一个问题'
    const first = service.submit(a.input), second = service.submit(b.input)
    b.complete(); expect(await second).toBe(true)
    expect(a.store.sendMessage).not.toHaveBeenCalled()
    a.complete(); expect(await first).toBe(true)
    expect(a.store.sendMessage.mock.calls[0][0]).toBe('引用\n原问题')
    expect(b.store.sendMessage.mock.calls[0][0]).toBe('另一个问题')
  })

  it.each(['close', 'context', 'conversation', 'model', 'mode', 'draft', 'quote', 'attachment', 'stop'] as const)(
    'cancels on %s without sending late results', async reason => {
      const c = setup(), service = new DeferredChatSubmissionService(), task = service.submit(c.input)
      if (reason === 'close') c.owner.release()
      if (reason === 'context') c.owner.invalidateContext()
      if (reason === 'conversation') c.store.currentConversationId = 'other'
      if (reason === 'model') c.state.modelId.value = 'other-model'
      if (reason === 'mode') c.state.dialogueCharacter.value.taskPlan = true
      if (reason === 'draft') c.state.messageStr.value = '后来输入'
      if (reason === 'quote') c.state.quotedText.value = '后来引用'
      if (reason === 'attachment') c.owner.removeAttachment('doc')
      if (reason === 'stop') expect(service.cancel(c.owner)).toBe(true)
      expect(await task).toBe(false)
      c.complete(); await nextTick()
      expect(c.store.sendMessage).not.toHaveBeenCalled()
      expect(c.state.pendingAutoSend.value).toBe(false)
      expect(useExecutionActivityStore().isTabProtected(c.input.tabPath)).toBe(false)
      if (reason === 'draft') expect(c.state.messageStr.value).toBe('后来输入')
    })

  it('reports a real upload failure even when the upload clears the pending flag, then allows retry', async () => {
    const c = setup(), service = new DeferredChatSubmissionService(), gate = deferred<any>()
    c.state.fileList.value = []
    const upload = uploadTabDocument(c.owner, new File(['doc'], 'file.txt'), { upload: () => gate.promise, persist: vi.fn() })
    const task = service.submit(c.input)
    gate.resolve({ success: false, message: '上传失败' })
    await upload
    expect(await task).toBe(false)
    expect(c.state.chatSubmissionError.value).toContain('附件处理失败')
    expect(c.state.messageStr.value).toBe('原问题')
    expect(c.store.sendMessage).not.toHaveBeenCalled()
    c.state.fileList.value[0].status = 'uploaded'
    expect(await service.submit(c.input)).toBe(true)
    expect(c.state.chatSubmissionError.value).toBe('')
  })

  it.each([false, true])('converts image previews and preserves OCR/vision semantics (vision=%s)', async vision => {
    const c = setup(), reader = vi.fn(async () => 'data:image/png;base64,A'), service = new DeferredChatSubmissionService(reader)
    c.input.vision = vision
    c.state.fileList.value = [{ id: 'image', name: 'image.png', type: 'image', status: 'pending', preview_url: 'blob:image', ocr_text: '文字' }]
    expect(await service.submit(c.input)).toBe(true)
    const args = c.store.sendMessage.mock.calls[0]
    expect(args[3][0].preview_url).toBe('data:image/png;base64,A')
    expect(args[12]).toEqual(vision ? ['data:image/png;base64,A'] : [])
    if (vision) expect(args[17]).toBeUndefined()
    else expect(args[17]).toContain('文字')
  })

  it('cancels promptly during image conversion and ignores its late completion', async () => {
    const c = setup(), gate = deferred<string>(), service = new DeferredChatSubmissionService(() => gate.promise)
    c.state.fileList.value = [{ id: 'image', name: 'image.png', type: 'image', status: 'pending', preview_url: 'blob:image' }]
    const task = service.submit(c.input)
    await nextTick(); service.cancel(c.owner)
    expect(await task).toBe(false)
    gate.resolve('data:image/png;base64,A'); await nextTick()
    expect(c.store.sendMessage).not.toHaveBeenCalled()
    expect(c.state.messageStr.value).toBe('原问题')
  })

  it('shows conversion errors without consuming the draft', async () => {
    const c = setup(), service = new DeferredChatSubmissionService(async () => { throw new Error('图片读取失败') })
    c.state.fileList.value = [{ id: 'image', name: 'image.png', type: 'image', status: 'pending', preview_url: 'blob:image' }]
    expect(await service.submit(c.input)).toBe(false)
    expect(c.state.chatSubmissionError.value).toBe('图片读取失败')
    expect(c.state.messageStr.value).toBe('原问题')
  })

  it('restores a consumed draft on Store preparation failure without overwriting a later draft', async () => {
    for (const edited of [false, true]) {
      const c = setup(), service = new DeferredChatSubmissionService()
      c.complete()
      c.store.sendMessage.mockImplementation(async () => {
        if (edited) c.state.messageStr.value = '后来输入'
        throw new Error('创建失败')
      })
      expect(await service.submit(c.input)).toBe(false)
      expect(c.state.messageStr.value).toBe(edited ? '后来输入' : '原问题')
      expect(c.state.chatSubmissionError.value).toBe('创建失败')
      if (!edited) expect(c.state.fileList.value).toHaveLength(1)
    }
  })
})

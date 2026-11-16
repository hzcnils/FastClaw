import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { FastPlanSessionManager } from './sessionManager'
import { useExecutionActivityStore } from '@/stores/executionActivity'
import { reactive, isRef, ref } from 'vue'

const invokeMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

beforeEach(() => {
  setActivePinia(createPinia())
  invokeMock.mockReset().mockResolvedValue(undefined)
})

function createFakeControllerFactory() {
  const controllers: any[] = []
  const factory = vi.fn((options: any) => {
    const controller = {
      options,
      state: {
        sessionId: options.sessionId,
        executionMode: options.executionMode || 'auto',
        executionStatus: 'running',
        treeCompleted: false,
        isPlanning: true,
        isPreviewing: false,
        isPaused: false,
        plannedTree: { id: 'tree-1' },
      },
      disconnect: vi.fn(),
      cancelPlan: vi.fn(),
      cancelExecution: vi.fn(),
      confirmPlan: vi.fn().mockResolvedValue(true),
      resumeExecution: vi.fn().mockReturnValue(true),
      pauseExecution: vi.fn().mockReturnValue(true),
      retryNode: vi.fn().mockResolvedValue(true),
      skipNode: vi.fn().mockReturnValue(true),
      respondHumanCheckpoint: vi.fn().mockResolvedValue(true),
      updateNodeOutput: vi.fn().mockReturnValue(true),
      updateNextNodeConfig: vi.fn().mockReturnValue(true),
      connect: vi.fn(),
      loading: { value: false },
      connected: { value: true },
      setExecutionMode: vi.fn((mode: string) => { controller.state.executionMode = mode }),
    }
    controllers.push(controller)
    return controller
  })
  return { factory, controllers }
}

describe('FastPlanSessionManager', () => {
  it('does not persist prompts, progress text or an older turn as the current final answer', async () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', { conversationId: 'conversation-1', tabPath: '/chat/a', entryId: 'entry-1' })
    const controller = controllers[0]
    controller.state.executionHistory = [{ treeId: 'old-tree', finalResult: { content: '旧结果' } }]
    controller.options.onEvent('state-change', { event: { type: 'chat', content: '用户问题' } }, controller.state)
    controller.options.onEvent('state-change', { event: { type: 'thinking', content: '正在分析' } }, controller.state)
    expect(invokeMock).not.toHaveBeenCalledWith('sync_desktop_plan_assistant_message', expect.anything())
    manager.disposeAll()
  })
  it('preserves runtime identity and Ref contracts through reactive plugin options', () => {
    const { factory } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    const config = reactive({ sessionManager: manager })
    const first = config.sessionManager.getOrCreate('reactive-session')
    first.connected = ref(true)
    const second = config.sessionManager.getOrCreate('reactive-session')
    expect(second).toBe(first)
    expect(isRef(second.connected)).toBe(true)
    manager.disposeAll()
  })

  it('delivers events from a controller created through reactive options to application owners', async () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    const config = reactive({ sessionManager: manager })
    config.sessionManager.getOrCreate('reactive-session')
    manager.bindSession('reactive-session', { conversationId: 'conversation-1', tabPath: '/chat/a', entryId: 'entry-1' })
    controllers[0].options.onEvent('state-change', { event: { type: 'route_stage_update', stage: 'intent', status: 'done' } }, controllers[0].state)
    manager.disposeAll()
  })
  function controlledSession() {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', { conversationId: 'conversation-1', tabPath: '/chat/a', entryId: 'entry-1' })
    Object.assign(controllers[0].state, { currentTreeId: 'tree-1', isPlanning: false, isPreviewing: true })
    return { manager, controller: controllers[0] }
  }

  it('snapshots node edits, coalesces identical clicks but preserves different edits, and survives view release', async () => {
    const { manager, controller } = controlledSession()
    controller.state.isPreviewing = false
    controller.state.executionState = { n: { id: 'n', status: 'completed' } }
    controller.state.plannedTree = { id: 'tree-1', root: { children: [{ id: 'phase', children: [{ id: 'n' }] }] } }
    manager.retain('session-1', 'view')
    const output = { value: 'original' }, changes = { prompt: 'original' }
    const first = manager.executeEntryControl('session-1', 'entry-1', { type: 'edit-node-output', nodeId: 'n', output })
    const duplicate = manager.executeEntryControl('session-1', 'entry-1', { type: 'edit-node-output', nodeId: 'n', output })
    const distinct = manager.executeEntryControl('session-1', 'entry-1', { type: 'edit-node-output', nodeId: 'n', output: { value: 'second' } })
    const config = manager.executeEntryControl('session-1', 'entry-1', { type: 'edit-node-config', nodeId: 'n', changes })
    output.value = 'late'; changes.prompt = 'late'; manager.release('session-1', 'view')
    expect(await Promise.all([first, duplicate, distinct, config])).toEqual([true, true, true, true])
    expect(controller.updateNodeOutput.mock.calls).toEqual([['n', { value: 'original' }], ['n', { value: 'second' }]])
    expect(controller.updateNextNodeConfig).toHaveBeenCalledExactlyOnceWith('n', { prompt: 'original' })
    manager.disposeAll()
  })

  it.each(['tree', 'close', 'stop', 'node'] as const)('rejects queued node edits after %s invalidation', async reason => {
    const { manager, controller } = controlledSession()
    controller.state.executionState = { n: { id: 'n', status: 'completed' } }
    controller.state.plannedTree = { id: 'tree-1', root: { children: [{ id: 'n' }] } }
    const target = manager.controlTarget('conversation-1', 'entry-1')!
    const task = manager.executeControl(target, { type: 'edit-node-output', nodeId: 'n', output: 'new' })
    if (reason === 'tree') controller.state.currentTreeId = 'new-tree'
    if (reason === 'close') manager.disposeAll()
    if (reason === 'stop') await manager.stopConversation('conversation-1', 'entry-1')
    if (reason === 'node') controller.state.executionState = {}
    expect(await task).toBe(false)
    expect(controller.updateNodeOutput).not.toHaveBeenCalled()
    manager.disposeAll()
  })

  it('routes edited confirmation by bound entry and snapshots payload before queuing', async () => {
    const { manager, controller } = controlledSession()
    const tree = { id: 'tree-1', metadata: { title: 'edited' } }
    const attachments = [{ path: '/selected.pdf', name: 'selected.pdf' }]
    const task = manager.executeEntryControl('session-1', 'entry-1', { type: 'confirm', tree, attachments })
    tree.metadata.title = 'later edit'; attachments[0].path = '/later.pdf'
    await expect(task).resolves.toBe(true)
    expect(controller.confirmPlan).toHaveBeenCalledWith({ id: 'tree-1', metadata: { title: 'edited' } },
      [{ path: '/selected.pdf', name: 'selected.pdf' }])
    expect(await manager.executeEntryControl('session-1', 'other-entry', { type: 'confirm' })).toBe(false)
    expect(await manager.executeEntryControl('other-session', 'entry-1', { type: 'confirm' })).toBe(false)
    manager.disposeAll()
  })

  it('rejects an edited tree from another turn without consuming the current confirmation', async () => {
    const { manager, controller } = controlledSession()
    expect(await manager.executeEntryControl('session-1', 'entry-1', { type: 'confirm', tree: { id: 'old-tree' } })).toBe(false)
    expect(controller.confirmPlan).not.toHaveBeenCalled()
    expect(await manager.executeEntryControl('session-1', 'entry-1', { type: 'confirm' })).toBe(true)
    manager.disposeAll()
  })

  it('snapshots and deduplicates human input, rejects resume and accepts the next same-node checkpoint', async () => {
    const { manager, controller } = controlledSession()
    const request = { tree_id: 'tree-1', node_id: 'node-1' }
    Object.assign(controller.state, { isPreviewing: false, isPaused: true, humanRequest: request, humanRequestQueue: [request] })
    const old = manager.controlTarget('conversation-1')!
    expect(old.waitNodeId).toBe('node-1')
    expect(await manager.executeControl(old, { type: 'resume' })).toBe(false)
    const response = { action: 'submit', values: { city: '西安' } }
    const first = manager.executeEntryControl('session-1', 'entry-1', { type: 'human-response', response })
    const second = manager.executeControl(old, { type: 'human-response', response })
    response.values.city = '重庆'
    await expect(first).resolves.toBe(true)
    await expect(second).resolves.toBe(true)
    expect(controller.respondHumanCheckpoint).toHaveBeenCalledExactlyOnceWith({ action: 'submit', values: { city: '西安' } }, request)
    expect(await manager.executeControl(old, { type: 'human-response', response })).toBe(false)
    const next = { ...request }
    Object.assign(controller.state, { humanRequest: next, humanRequestQueue: [next] })
    expect(await manager.executeControl(old, { type: 'human-response', response })).toBe(false)
    expect(await manager.executeEntryControl('session-1', 'entry-1', { type: 'human-response', response, request })).toBe(false)
    expect(await manager.executeEntryControl('session-1', 'entry-1', { type: 'human-response', response })).toBe(true)
    expect(controller.respondHumanCheckpoint).toHaveBeenCalledTimes(2)
    manager.disposeAll()
  })

  it('does not consume a failed checkpoint and rejects a queued response when the checkpoint changes', async () => {
    const { manager, controller } = controlledSession()
    Object.assign(controller.state, { isPreviewing: false, humanRequest: { tree_id: 'tree-1', node_id: 'node-1' } })
    const command = { type: 'human-response' as const, response: { action: 'continue' } }
    controller.respondHumanCheckpoint.mockRejectedValueOnce(new Error('offline'))
    await expect(manager.executeEntryControl('session-1', 'entry-1', command)).rejects.toThrow('offline')
    expect(await manager.executeEntryControl('session-1', 'entry-1', command)).toBe(true)
    controller.state.humanRequest = { tree_id: 'tree-1', node_id: 'node-2' }
    const queued = manager.executeEntryControl('session-1', 'entry-1', command)
    controller.state.humanRequest = { tree_id: 'tree-1', node_id: 'node-3' }
    expect(await queued).toBe(false)
    expect(controller.respondHumanCheckpoint).toHaveBeenCalledTimes(2)
    manager.disposeAll()
  })

  it.each(['cancel-plan', 'cancel-execution'] as const)('%s invalidates queued confirmation synchronously even when offline', async type => {
    const { manager, controller } = controlledSession()
    controller.cancelPlan.mockImplementation(() => { throw new Error('offline') })
    controller.cancelExecution.mockImplementation(() => { throw new Error('offline') })
    const queued = manager.executeEntryControl('session-1', 'entry-1', { type: 'confirm' })
    const stop = manager.executeEntryControl('session-1', 'entry-1', { type })
    expect(type === 'cancel-plan' ? controller.cancelPlan : controller.cancelExecution).toHaveBeenCalledTimes(1)
    await expect(stop).rejects.toThrow('offline')
    expect(await queued).toBe(false)
    expect(controller.confirmPlan).not.toHaveBeenCalled()
    manager.disposeAll()
  })

  it('coalesces different clicks for the same in-flight wait and consumes it once', async () => {
    const { manager, controller } = controlledSession()
    let resolve!: (ok: boolean) => void
    controller.confirmPlan.mockReturnValue(new Promise<boolean>(yes => { resolve = yes }))
    const target = manager.controlTarget('conversation-1')!
    const first = manager.executeControl(target, { type: 'confirm' })
    const second = manager.executeControl({ ...target }, { type: 'confirm' })
    await Promise.resolve()
    expect(controller.confirmPlan).toHaveBeenCalledTimes(1)
    resolve(true)
    expect(await first).toBe(true)
    expect(await second).toBe(true)
    expect(await manager.executeControl(target, { type: 'confirm' })).toBe(false)
    manager.disposeAll()
  })

  it('allows retrying a failed confirmation but rejects a stale tree or turn', async () => {
    const { manager, controller } = controlledSession()
    const target = manager.controlTarget('conversation-1')!
    controller.confirmPlan.mockRejectedValueOnce(new Error('offline'))
    await expect(manager.executeControl(target, { type: 'confirm' })).rejects.toThrow('offline')
    expect(await manager.executeControl(target, { type: 'confirm' })).toBe(true)
    controller.state.currentTreeId = 'tree-2'
    expect(await manager.executeControl(target, { type: 'confirm' })).toBe(false)
    expect(controller.confirmPlan).toHaveBeenCalledTimes(2)
    manager.disposeAll()
  })

  it('rejects a queued command if its session closes and the same ID reopens', async () => {
    const { manager, controller } = controlledSession()
    const target = manager.controlTarget('conversation-1')!
    const operation = manager.executeControl(target, { type: 'confirm' })
    manager.disposeSession('session-1')
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', { conversationId: 'conversation-1', tabPath: '/chat/a', entryId: 'entry-1' })
    expect(await operation).toBe(false)
    expect(controller.confirmPlan).not.toHaveBeenCalled()
    controller.options.onEvent('state-change', { event: { type: 'tree_started' } }, controller.state)
    expect(invokeMock).not.toHaveBeenCalledWith('emit_desktop_plan_events', expect.anything())
    manager.disposeAll()
  })

  it('observes successive step waits while no UI is subscribed', async () => {
    const { manager, controller } = controlledSession()
    Object.assign(controller.state, { isPreviewing: false, isPaused: true, lastCompletedNodeId: 'node-1' })
    const old = manager.controlTarget('conversation-1')!
    expect(await manager.executeControl(old, { type: 'resume' })).toBe(true)
    controller.state.isPaused = false
    controller.options.onEvent('state-change', {}, controller.state)
    controller.state.isPaused = true
    controller.options.onEvent('state-change', {}, controller.state)
    expect(await manager.executeControl(old, { type: 'resume' })).toBe(false)
    expect(await manager.executeControl(manager.controlTarget('conversation-1')!, { type: 'resume' })).toBe(true)
    expect(controller.resumeExecution).toHaveBeenCalledTimes(2)
    manager.disposeAll()
  })

  it('does not reconnect a connected runtime on view reactivation', async () => {
    const { manager, controller } = controlledSession()
    await manager.executeControl(manager.controlTarget('conversation-1')!, { type: 'ensure-live' })
    expect(controller.connect).not.toHaveBeenCalled()
    manager.disposeAll()
  })

  it.each(['retry-node', 'skip-node'] as const)('%s keeps its captured node and deduplicates pending clicks', async type => {
    const { manager, controller } = controlledSession()
    Object.assign(controller.state, { isPreviewing: false, executionState: { node: { status: 'failed' } } })
    const method = type === 'retry-node' ? controller.retryNode : controller.skipNode
    let resolve!: (value: boolean) => void
    method.mockReturnValue(new Promise<boolean>(done => { resolve = done }))
    const target = manager.controlTarget('conversation-1', 'entry-1')!
    const first = manager.executeControl(target, { type, nodeId: 'node' })
    const duplicate = manager.executeControl(target, { type, nodeId: 'node' })
    expect(duplicate).toBe(first)
    await Promise.resolve(); expect(method).toHaveBeenCalledExactlyOnceWith('node')
    resolve(true); expect(await first).toBe(true)
    expect(await manager.executeControl(target, { type, nodeId: 'missing' })).toBe(false)
    manager.disposeAll()
  })

  it.each(['retry-node', 'skip-node'] as const)('%s refuses a queued command after the tree changes', async type => {
    const { manager, controller } = controlledSession()
    Object.assign(controller.state, { isPreviewing: false, executionState: { node: { status: 'failed' } } })
    const task = manager.executeControl(manager.controlTarget('conversation-1', 'entry-1')!, { type, nodeId: 'node' })
    controller.state.currentTreeId = 'replacement-tree'
    expect(await task).toBe(false)
    expect(controller.retryNode).not.toHaveBeenCalled(); expect(controller.skipNode).not.toHaveBeenCalled()
    manager.disposeAll()
  })

  it('reuses one controller for the same live session', () => {
    const { factory } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)

    const first = manager.getOrCreate('session-1', { executionMode: 'auto' })
    const second = manager.getOrCreate('session-1', { executionMode: 'step' })

    expect(second).toBe(first)
    expect(factory).toHaveBeenCalledTimes(1)
    expect(first.setExecutionMode).toHaveBeenCalledWith('step')
    expect(manager.stats()).toEqual({ total: 1, active: 1, retained: 0 })
  })

  it('does not disconnect an active session when its renderer is released', () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.retain('session-1', 'entry-1')

    manager.release('session-1', 'entry-1')

    expect(controllers[0].disconnect).not.toHaveBeenCalled()
    expect(manager.has('session-1')).toBe(true)
  })

  it('disconnects only when explicitly disposed', () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')

    manager.disposeSession('session-1')

    expect(controllers[0].disconnect).toHaveBeenCalledTimes(1)
    expect(manager.has('session-1')).toBe(false)
  })

  it('cancels and disposes an unmounted session when its tab closes', async () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', {
      conversationId: 'conversation-1',
      tabPath: '/chat/tab-1',
      entryId: 'entry-1',
    })

    await manager.closeTabSessions('/chat/tab-1', true)

    expect(controllers[0].cancelPlan).toHaveBeenCalledTimes(1)
    expect(controllers[0].disconnect).toHaveBeenCalledTimes(1)
    expect(manager.has('session-1')).toBe(false)
  })

  it('reclaims a terminal controller after its last renderer is gone', () => {
    vi.useFakeTimers()
    try {
      const { factory, controllers } = createFakeControllerFactory()
      const manager = new FastPlanSessionManager(factory as any)
      manager.getOrCreate('session-1')
      manager.retain('session-1', 'entry-1')
      manager.release('session-1', 'entry-1')
      Object.assign(controllers[0].state, {
        executionStatus: 'completed', treeCompleted: true, isPlanning: false,
      })

      controllers[0].options.onEvent('execution-completed', {}, controllers[0].state)
      vi.advanceTimersByTime(30_000)

      expect(controllers[0].disconnect).toHaveBeenCalledTimes(1)
      expect(manager.has('session-1')).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })

  it('keeps writer ownership in the application-level manager', () => {
    const { factory } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)

    expect(manager.acquireWriter('conversation-1', 'tab-a')).toBe(true)
    expect(manager.acquireWriter('conversation-1', 'tab-b')).toBe(false)
    manager.releaseWriter('conversation-1', 'tab-a')
    expect(manager.acquireWriter('conversation-1', 'tab-b')).toBe(true)
  })

  it('deduplicates terminal synchronization across renderer remounts', () => {
    const { factory } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)

    expect(manager.claimTerminalSync('session-1', 'tree-1')).toBe(true)
    expect(manager.claimTerminalSync('session-1', 'tree-1')).toBe(false)
    expect(manager.claimTerminalSync('session-1', 'tree-2')).toBe(true)
    manager.releaseTerminalSync('session-1', 'tree-1')
    expect(manager.claimTerminalSync('session-1', 'tree-1')).toBe(true)
  })

  it('keeps execution activity authoritative after the renderer releases', () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', {
      conversationId: 'conversation-1',
      tabPath: '/chat/tab-1',
      entryId: 'entry-1',
      executionMode: 'step',
    })
    manager.retain('session-1', 'renderer-1')
    manager.release('session-1', 'renderer-1')

    Object.assign(controllers[0].state, {
      isPlanning: false,
      isPaused: true,
      executionMode: 'step',
    })
    controllers[0].options.onEvent('state-change', {}, controllers[0].state)

    expect(useExecutionActivityStore().activityByConversation('conversation-1')).toMatchObject({
      phase: 'awaiting_step_confirmation',
      protected: true,
      sessionId: 'session-1',
      entryId: 'entry-1',
    })
    expect(controllers[0].disconnect).not.toHaveBeenCalled()
  })

  it('mirrors controller events after the UI owner is released', async () => {
    invokeMock.mockClear()
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', {
      conversationId: 'conversation-1',
      tabPath: '/chat/tab-1',
      entryId: 'entry-1',
    })

    controllers[0].options.onEvent('state-change', {
      event: { type: 'tree_started', tree_id: 'tree-1' },
    }, controllers[0].state)

    manager.disposeAll()
  })

  it('persists a terminal result after the renderer is released', async () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', {
      conversationId: 'conversation-1',
      tabPath: '/chat/tab-1',
      entryId: 'entry-1',
      modelId: 'model-1',
      groupId: 'group-1',
    })
    manager.retain('session-1', 'renderer-1')
    manager.release('session-1', 'renderer-1')
    Object.assign(controllers[0].state, {
      isPlanning: false,
      treeCompleted: true,
      executionStatus: 'completed',
      currentTreeId: 'tree-1',
      finalResult: { content: 'done' },
    })

    controllers[0].options.onEvent('state-change', {
      event: { type: 'tree_completed', tree_id: 'tree-1' },
    }, controllers[0].state)

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'sync_desktop_plan_assistant_message',
      expect.objectContaining({
        conversationId: 'conversation-1',
        content: 'done',
        groupId: 'group-1',
        modelId: 'model-1',
        fastplanSessionId: 'session-1',
        entryId: 'entry-1',
        completionId: 'tree-1',
        terminalStatus: 'complete',
      }),
    ))
    expect(controllers[0].disconnect).not.toHaveBeenCalled()
    manager.disposeAll()
  })

  it('waits for execution completion instead of saving an empty tree-completed answer', async () => {
    const { factory, controllers } = createFakeControllerFactory()
    const manager = new FastPlanSessionManager(factory as any)
    manager.getOrCreate('session-1')
    manager.bindSession('session-1', {
      conversationId: 'conversation-1',
      tabPath: '/chat/tab-1',
      entryId: 'entry-1',
    })
    Object.assign(controllers[0].state, {
      isPlanning: false,
      treeCompleted: true,
      executionStatus: 'completed',
      currentTreeId: 'tree-1',
      finalResult: null,
    })

    controllers[0].options.onEvent('state-change', {
      event: { type: 'tree_completed', tree_id: 'tree-1' },
    }, controllers[0].state)
    await Promise.resolve()
    expect(invokeMock).not.toHaveBeenCalledWith(
      'sync_desktop_plan_assistant_message',
      expect.anything(),
    )

    controllers[0].options.onEvent('execution-completed', {
      event: { type: 'tree_completed', tree_id: 'tree-1' },
    }, controllers[0].state)
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'sync_desktop_plan_assistant_message',
      expect.objectContaining({ completionId: 'tree-1' }),
    ))
    manager.disposeAll()
  })
})

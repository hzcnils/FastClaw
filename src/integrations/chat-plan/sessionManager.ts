import {
  createFastPlanExecutionController,
  type FastPlanAccess,
  type FastPlanExecutionController,
  type FastPlanExecutionControllerOptions,
} from 'planflow-runtime'
import {
  useExecutionActivityStore,
  type ExecutionPhase,
} from '@/stores/executionActivity'
import { chatSwitchPerf } from '@/utils/chatSwitchPerf'
import { markRaw } from 'vue'
import { acknowledgeFastPlanSend } from './tabRuntimeBridge'
import type { FastPlanInteractionCommand } from 'planflow-kit'

type ControllerFactory = (options: FastPlanExecutionControllerOptions) => FastPlanExecutionController

interface ManagedFastPlanSession {
  controller: FastPlanExecutionController
  access?: FastPlanAccess
  onEvent?: FastPlanExecutionControllerOptions['onEvent']
  owners: Set<string>
  lastAccessedAt: number
  disposeTimer: ReturnType<typeof setTimeout> | null
  revision: number
  waitRevision: number
  waitKey: string
  commands: Map<string, Promise<boolean>>
  consumedWaits: Set<string>
}

export interface FastPlanControlTarget {
  sessionId: string
  conversationId: string
  entryId: string
  revision: number
  treeId: string
  turnId: string
  waitRevision: number
  waitNodeId: string
}

export interface FastPlanControlCommand extends Omit<FastPlanInteractionCommand, 'type'> {
  type: FastPlanInteractionCommand['type'] | 'ensure-live'
}

export interface FastPlanSessionBinding {
  conversationId: string
  tabPath: string
  entryId: string
  executionMode?: 'auto' | 'manual' | 'step'
  modelId?: string | null
  groupId?: string | null
}

const TERMINAL_RELEASE_DELAY_MS = 30_000
const MAX_RELEASED_SESSIONS = 8
const COALESCED_DESKTOP_EVENTS = new Set([
  'thinking',
  'thinking_update',
  'execution_log',
  'node_progress',
  'node_update',
  'execution_progress',
  'task_progress',
])

function isTerminalSession(controller: FastPlanExecutionController) {
  const state = controller.state
  const status = String(state.executionStatus || '').toLowerCase()
  return !!state.treeCompleted
    || ['completed', 'failed', 'cancelled'].includes(status)
    || !!state.planningError
    || Object.values(state.executionState || {}).some(
      (node: any) => String(node?.status || '').toLowerCase() === 'failed',
    )
}

function terminalStatus(
  name: string,
  payload: any,
  state: FastPlanExecutionController['state'],
): 'complete' | 'failed' | null {
  const status = String(state.executionStatus || payload?.executionStatus || '').toLowerCase()
  const failed = name === 'error'
    || payload?.success === false
    || status === 'failed'
    || status === 'cancelled'
    || !!state.planningError
    || Object.values(state.executionState || {}).some(
      (node: any) => String(node?.status || '').toLowerCase() === 'failed',
  )
  if (failed) return 'failed'
  // Progress and outgoing chat frames also carry `content`; neither is a final answer.
  const finalBoundary = payload?.event?.type === 'final_answer' || state.treeCompleted || status === 'completed'
  const hasFinalResult = finalBoundary && !!extractFinalAnswer(state, payload?.event)
  if (name === 'execution-completed' || hasFinalResult) return 'complete'
  if (name === 'binding-restored' && (state.treeCompleted || status === 'completed')) return 'complete'
  return null
}

function buildPlanRuntime(state: any): Record<string, unknown> | null {
  if (!state || typeof state !== 'object') return null
  return {
    plannedTree: state.plannedTree ?? state.planned_tree ?? null,
    executionState: state.executionState ?? state.execution_state ?? {},
    validationFlow: state.validationFlow ?? state.validation_flow ?? [],
    routeFlow: state.routeFlow ?? state.route_flow ?? [],
    thinkingSteps: state.thinkingSteps ?? state.thinking_steps ?? [],
    thinkingStream: state.thinkingStream ?? state.thinking_stream ?? '',
    treeCompleted: !!(state.treeCompleted ?? state.tree_completed),
    executionStatus: state.executionStatus ?? state.execution_status ?? null,
    currentTreeId: state.currentTreeId ?? state.current_tree_id ?? '',
    planningError: state.planningError ?? state.planning_error ?? null,
    isPlanning: !!(state.isPlanning ?? state.is_planning),
    isPreviewing: !!(state.isPreviewing ?? state.is_previewing),
    isPaused: !!(state.isPaused ?? state.is_paused),
    lastCompletedNodeId: state.lastCompletedNodeId ?? state.last_completed_node_id ?? null,
    stepConfirmNodeId: state.stepConfirmNodeId ?? state.step_confirm_node_id ?? null,
    executionMode: state.executionMode ?? state.execution_mode ?? 'auto',
    presentation: state.presentation ?? null,
    taskBubbles: state.taskBubbles ?? state.task_bubbles ?? [],
    finalResult: state.finalResult ?? state.final_result ?? null,
  }
}

function extractFinalAnswer(state: any, event?: Record<string, unknown>): string {
  const finalFrame = ['final_answer', 'tree_completed', 'execution_completed'].includes(String(event?.type || ''))
  const fromEvent = finalFrame ? String(event?.content || event?.final_output || '').trim() : ''
  if (fromEvent) return fromEvent
  const finalResult = state?.finalResult || state?.final_result
  if (finalResult && typeof finalResult === 'object') {
    const text = String(finalResult.content || finalResult.final_output || '').trim()
    if (text) return text
  }
  if (typeof finalResult === 'string' && finalResult.trim()) return finalResult.trim()
  const history = Array.isArray(state?.executionHistory) ? state.executionHistory : []
  for (let index = history.length - 1; index >= 0; index -= 1) {
    const item = history[index]
    if (!state?.currentTreeId || String(item?.treeId || item?.tree_id || '') !== String(state.currentTreeId)) continue
    const text = String(item?.finalResult?.content || item?.final_result?.content || '').trim()
    if (text) return text
  }
  return ''
}

function executionPhase(state: FastPlanExecutionController['state']): ExecutionPhase {
  const connectionStatus = String(state.connectionStatus || '').toLowerCase()
  if (connectionStatus === 'reconnecting' || connectionStatus === 'connecting') return 'reconnecting'
  if (state.humanRequest || state.humanRequestQueue?.length || state.humanInterventionRequired) return 'awaiting_human'
  if (state.isPreviewing) return 'awaiting_plan_confirmation'
  if (state.isPaused && state.executionMode === 'step') return 'awaiting_step_confirmation'
  if (state.isPaused) return 'paused'
  if (state.plannedTree && !state.treeCompleted) return 'executing'
  if (state.isPlanning) return 'planning'
  const status = String(state.executionStatus || '').toLowerCase()
  if (status === 'completed') return 'completed'
  if (status === 'failed') return 'failed'
  if (status === 'cancelled') return 'cancelled'
  return 'idle'
}

/**
 * Owns live FastPlan controllers outside ChatView/KeepAlive.
 *
 * A renderer may disappear while a task keeps running. Releasing a renderer
 * therefore only removes its UI owner; it never disconnects an active session.
 */
export class FastPlanSessionManager {
  private revision = 0
  private checkpointIds = new WeakMap<object, number>()
  private checkpointRevision = 0
  private readonly sessions = new Map<string, ManagedFastPlanSession>()
  private readonly bindings = new Map<string, FastPlanSessionBinding>()
  private readonly writers = new Map<string, string>()
  private readonly submissions = new Map<string, { binding: FastPlanSessionBinding; conversationIds: Set<string>; cancel: () => void; accepted: boolean }>()
  private readonly pendingDesktopEvents = new Map<string, Map<string, Record<string, unknown>>>()
  private readonly pendingDesktopTimers = new Map<string, ReturnType<typeof setTimeout>>()
  private readonly desktopEventChains = new Map<string, Promise<void>>()
  private readonly desktopEventSignatures = new Map<string, { signature: string; at: number }>()
  private readonly desktopWaitKeys = new Map<string, string>()
  private readonly terminalSyncClaims = new Set<string>()

  constructor(private readonly createController: ControllerFactory = createFastPlanExecutionController) {
    // Service identity, Maps and controller Ref contracts must survive reactive plugin options.
    // The controller's state remains reactive; the service itself is not presentation state.
    markRaw(this)
  }

  getOrCreate(
    sessionId: string,
    options: Omit<FastPlanExecutionControllerOptions, 'sessionId' | 'autoConnect'> = {},
  ): FastPlanExecutionController {
    const id = sessionId.trim()
    if (!id) throw new Error('FastPlan sessionId is required')

    const existing = this.sessions.get(id)
    if (existing) {
      existing.access = options.access
      existing.onEvent = options.onEvent
      existing.lastAccessedAt = Date.now()
      if (options.executionMode && existing.controller.state.executionMode !== options.executionMode) {
        existing.controller.setExecutionMode(options.executionMode)
      }
      return existing.controller
    }

    // Keep credential/model resolution replaceable. A controller can outlive
    // the ChatView that first created it, so it must not retain that view's
    // access closure forever.
    const managed: ManagedFastPlanSession = {
      controller: null as unknown as FastPlanExecutionController,
      access: options.access,
      onEvent: options.onEvent,
      owners: new Set(),
      lastAccessedAt: Date.now(),
      disposeTimer: null,
      revision: ++this.revision,
      waitRevision: 0,
      waitKey: '',
      commands: new Map(),
      consumedWaits: new Set(),
    }
    const access: FastPlanAccess | undefined = options.access
      ? {
          getConnection: () => {
            if (!managed.access) throw new Error('FastPlan access is unavailable')
            return managed.access.getConnection()
          },
          resolveModel: selection => {
            if (!managed.access) throw new Error('FastPlan access is unavailable')
            return managed.access.resolveModel(selection)
          },
        }
      : undefined
    managed.controller = markRaw(this.createController({
      ...options,
      sessionId: id,
      access,
      autoConnect: false,
      persistUserMessages: false,
      onEvent: (name, payload, state) => {
        // An already queued transport event must not affect a replacement session.
        if (this.sessions.get(id) !== managed) return
        if (name === 'message-sent') {
          const binding = this.bindings.get(id)
          if (binding) {
            const submission = this.submissions.get(`${binding.tabPath}:${binding.entryId}`)
            if (submission) submission.accepted = true
            acknowledgeFastPlanSend(binding, id, String(payload?.request_id || ''))
          }
        }
        managed.lastAccessedAt = Date.now()
        managed.onEvent?.(name, payload, state)
        this.syncExecutionActivity(id, state)
        if (name === 'state-change') this.mirrorControllerState(id, payload, state)
        void this.syncTerminalState(id, name, payload, state)
        this.scheduleTerminalRelease(id, managed)
      },
    }))
    this.sessions.set(id, managed)
    this.pruneReleasedSessions()
    return managed.controller
  }

  retain(sessionId: string, ownerId: string) {
    const managed = this.sessions.get(sessionId)
    if (!managed || !ownerId) return
    if (managed.disposeTimer) clearTimeout(managed.disposeTimer)
    managed.disposeTimer = null
    managed.owners.add(ownerId)
    managed.lastAccessedAt = Date.now()
  }

  release(sessionId: string, ownerId: string) {
    const managed = this.sessions.get(sessionId)
    if (!managed) return
    managed.owners.delete(ownerId)
    managed.lastAccessedAt = Date.now()
    this.scheduleTerminalRelease(sessionId, managed)
  }

  private scheduleTerminalRelease(sessionId: string, managed: ManagedFastPlanSession) {
    if (managed.owners.size || !isTerminalSession(managed.controller)) return
    if (managed.disposeTimer) clearTimeout(managed.disposeTimer)
    managed.disposeTimer = setTimeout(() => {
      const current = this.sessions.get(sessionId)
      if (current === managed && current.owners.size === 0 && isTerminalSession(current.controller)) {
        this.disposeSession(sessionId)
      }
    }, TERMINAL_RELEASE_DELAY_MS)
  }

  get(sessionId: string) {
    return this.sessions.get(sessionId)?.controller
  }

  /** A value snapshot for commands; contains no view, route, credentials or mutable state. */
  controlTarget(conversationId: string, entryId?: string): FastPlanControlTarget | null {
    const candidates = [...this.bindings.entries()].filter(([id, binding]) =>
      binding.conversationId === conversationId && (!entryId || binding.entryId === entryId)
      && this.sessions.has(id)
      && (entryId || !isTerminalSession(this.sessions.get(id)!.controller)))
    // Ambiguous history must never silently target another active turn.
    if (candidates.length !== 1) return null
    const [sessionId, binding] = candidates[0]
    const managed = this.sessions.get(sessionId)!
    const state = managed.controller.state
    const treeId = String(state.currentTreeId || state.plannedTree?.id || state.plannedTree?.tree_id || '')
    const turnId = String(state.activeUserMessageId || '')
    const checkpoint = state.humanRequestQueue?.[0] || state.humanRequest
    if (checkpoint && !this.checkpointIds.has(checkpoint)) this.checkpointIds.set(checkpoint, ++this.checkpointRevision)
    const waitNodeId = String(checkpoint?.node_id || state.lastCompletedNodeId || state.stepConfirmNodeId || '')
    const waitKey = `${turnId}:${treeId}:${checkpoint ? `human:${this.checkpointIds.get(checkpoint)}`
      : state.isPreviewing ? 'preview' : state.isPaused ? `paused:${waitNodeId}` : 'running'}`
    if (managed.waitKey !== waitKey) {
      managed.waitKey = waitKey
      managed.waitRevision++
      managed.consumedWaits.clear()
    }
    return { sessionId, conversationId, entryId: binding.entryId, revision: managed.revision,
      treeId, turnId, waitRevision: managed.waitRevision, waitNodeId }
  }

  /** Public-package bridge: never guess the conversation from the currently visible Tab. */
  executeEntryControl(sessionId: string, entryId: string, command: FastPlanInteractionCommand): Promise<boolean> {
    const binding = this.bindings.get(sessionId)
    if (!binding || binding.entryId !== entryId) return Promise.resolve(false)
    const target = this.controlTarget(binding.conversationId, entryId)
    return target?.sessionId === sessionId ? this.executeControl(target, command) : Promise.resolve(false)
  }

  /** UI release is deliberately absent: only command identity/close/terminal state can invalidate it. */
  executeControl(target: FastPlanControlTarget, command: FastPlanControlCommand): Promise<boolean> {
    const managed = this.sessions.get(target.sessionId)
    const matches = () => {
      if (!managed || this.sessions.get(target.sessionId) !== managed) return false
      const current = this.controlTarget(target.conversationId, target.entryId)
      return !!current && current.sessionId === target.sessionId && current.revision === target.revision
        && current.treeId === target.treeId && current.turnId === target.turnId
        && current.waitRevision === target.waitRevision && current.waitNodeId === target.waitNodeId
    }
    if (!managed || !matches()) return Promise.resolve(false)
    if (command.type === 'cancel-plan' || command.type === 'cancel-execution') {
      return this.stopConversation(target.conversationId, target.entryId,
        command.type === 'cancel-plan' ? 'plan' : 'execution')
    }
    const checkpoint = managed.controller.state.humanRequestQueue?.[0] || managed.controller.state.humanRequest
    if (command.type === 'human-response' && (!checkpoint || (command.request && command.request !== checkpoint))) return Promise.resolve(false)
    if (command.type === 'confirm' && command.tree) {
      const id = String(command.tree.id || command.tree.tree_id || '')
      if (id && id !== target.treeId) return Promise.resolve(false)
    }
    let key = `${target.revision}:${target.waitRevision}:${command.type}:${command.nodeId || ''}`
    // Clone before scheduling. UI editing after the click cannot mutate the accepted intent.
    try {
      command = { ...command,
        tree: command.tree === undefined ? undefined : JSON.parse(JSON.stringify(command.tree)),
        response: command.response === undefined ? undefined : JSON.parse(JSON.stringify(command.response)),
        output: command.output === undefined ? undefined : JSON.parse(JSON.stringify(command.output)),
        changes: command.changes === undefined ? undefined : JSON.parse(JSON.stringify(command.changes)),
        attachments: command.attachments?.map(file => ({ ...file })),
      }
      if (command.type === 'edit-node-output' || command.type === 'edit-node-config') {
        key += `:${JSON.stringify([command.output, command.changes])}`
      }
    } catch (error) { return Promise.reject(error) }
    const pending = managed.commands.get(key)
    if (pending) return pending
    if (managed.consumedWaits.has(key)) return Promise.resolve(false)
    const operation = Promise.resolve().then(async () => {
      if (!matches()) return false
      const controller = managed.controller
      const state = controller.state
      if (!['ensure-live', 'retry-node', 'skip-node'].includes(command.type) && isTerminalSession(controller)) return false
      let result: unknown
      switch (command.type) {
        case 'confirm':
          if (!state.isPreviewing || !state.plannedTree) return false
          result = await controller.confirmPlan(command.tree, command.attachments)
          break
        case 'resume':
          if (!state.isPaused || !state.currentTreeId || checkpoint) return false
          result = await controller.resumeExecution()
          break
        case 'pause':
          if (state.isPaused || state.isPreviewing || !state.currentTreeId) return false
          result = await controller.pauseExecution()
          break
        case 'retry-node':
        case 'skip-node':
          if (!command.nodeId || !state.currentTreeId || !state.executionState?.[command.nodeId]) return false
          result = command.type === 'retry-node'
            ? await controller.retryNode(command.nodeId) : await controller.skipNode(command.nodeId)
          break
        case 'human-response':
          if (!checkpoint || !command.response) return false
          result = await controller.respondHumanCheckpoint(command.response, checkpoint)
          break
        case 'edit-node-output':
          if (!command.nodeId || !state.currentTreeId || !state.executionState?.[command.nodeId]) return false
          result = await controller.updateNodeOutput(command.nodeId, command.output)
          break
        case 'edit-node-config': {
          const containsNode = (node: any): boolean => !!node && (node.id === command.nodeId || node.children?.some(containsNode))
          if (!command.nodeId || !state.currentTreeId || !command.changes || !containsNode(state.plannedTree?.root)) return false
          result = await controller.updateNextNodeConfig(command.nodeId, command.changes)
          break
        }
        case 'ensure-live':
          if (!controller.loading.value && !controller.connected.value) controller.connect(target.sessionId)
          return true
      }
      if (result === false) return false
      if (this.sessions.get(target.sessionId) !== managed || managed.revision !== target.revision) return false
      // Different click IDs cannot advance the same confirmation twice.
      if (command.type === 'confirm' || command.type === 'resume' || command.type === 'human-response') managed.consumedWaits.add(key)
      this.syncExecutionActivity(target.sessionId, state)
      return true
    }).finally(() => { if (managed.commands.get(key) === operation) managed.commands.delete(key) })
    managed.commands.set(key, operation)
    return operation
  }

  bindSession(sessionId: string, binding: FastPlanSessionBinding) {
    const id = sessionId.trim()
    if (!id || !binding.conversationId || !binding.tabPath || !binding.entryId) return
    this.bindings.set(id, { ...this.bindings.get(id), ...binding })
    const managed = this.sessions.get(id)
    if (managed) {
      this.syncExecutionActivity(id, managed.controller.state)
      void this.syncTerminalState(id, 'binding-restored', {}, managed.controller.state)
    }
  }

  updateSessionMetadata(
    sessionId: string,
    metadata: Pick<FastPlanSessionBinding, 'modelId' | 'groupId'>,
  ) {
    const binding = this.bindings.get(sessionId)
    if (!binding) return
    this.bindings.set(sessionId, { ...binding, ...metadata })
  }

  markSubmitting(sessionId: string) {
    const binding = this.bindings.get(sessionId)
    if (!binding) return
    useExecutionActivityStore().update(binding.conversationId, binding.tabPath, 'fastplan', {
      phase: 'submitting',
      protected: true,
      sessionId,
      entryId: binding.entryId,
      executionMode: binding.executionMode,
    })
  }

  /** Pending creation/upload belongs to the application, including before a controller exists. */
  registerSubmission(binding: FastPlanSessionBinding, cancel: () => void, conversationAliases: string[] = []) {
    const key = `${binding.tabPath}:${binding.entryId}`
    if (this.submissions.has(key)) throw new Error('该规划请求仍在提交中')
    const record = { binding, conversationIds: new Set([binding.conversationId, ...conversationAliases]), cancel, accepted: false }
    this.submissions.set(key, record)
    return () => { if (this.submissions.get(key) === record) this.submissions.delete(key) }
  }

  hasSubmission(conversationId: string, entryId?: string) {
    return [...this.submissions.values()].some(item => item.conversationIds.has(conversationId)
      && (!entryId || item.binding.entryId === entryId))
  }

  refreshExecutionActivity(sessionId: string) {
    const runtime = this.get(sessionId)
    if (runtime) this.syncExecutionActivity(sessionId, runtime.state)
  }

  async stopConversation(conversationId: string, entryId?: string, kind?: 'plan' | 'execution'): Promise<boolean> {
    const pending = [...this.submissions.values()].filter(item => item.conversationIds.has(conversationId)
      && (!entryId || item.binding.entryId === entryId))
    // Invalidate creation/upload/model wait synchronously, before sending any stop packet.
    for (const item of pending) item.cancel()
    const targets = [...this.bindings.entries()].filter(([, binding]) => binding.conversationId === conversationId
      && (!entryId || binding.entryId === entryId))
    for (const [id, binding] of targets) {
      const managed = this.sessions.get(id)
      if (!managed) continue
      // Stop takes priority over control jobs already queued for the same runtime.
      managed.revision = ++this.revision
      const controller = managed.controller
      const submitting = pending.some(item => item.binding.entryId === binding.entryId)
      if (!submitting && isTerminalSession(controller)) continue
      const state = controller.state
      if (submitting || kind === 'plan' || (!kind && (state.isPreviewing || state.isPlanning || !state.currentTreeId))) controller.cancelPlan()
      else controller.cancelExecution()
      this.syncExecutionActivity(id, state)
    }
    return pending.length > 0 || targets.length > 0
  }

  has(sessionId: string) {
    return this.sessions.has(sessionId)
  }

  disposeSession(sessionId: string) {
    const managed = this.sessions.get(sessionId)
    if (!managed) return
    const binding = this.bindings.get(sessionId)
    if (managed.disposeTimer) clearTimeout(managed.disposeTimer)
    managed.controller.disconnect()
    this.sessions.delete(sessionId)
    this.bindings.delete(sessionId)
    if (binding) useExecutionActivityStore().remove(binding.conversationId, 'fastplan')
    for (const key of this.terminalSyncClaims) {
      if (key.startsWith(`${sessionId}:`)) this.terminalSyncClaims.delete(key)
    }
  }

  disposeAll() {
    for (const item of [...this.submissions.values()]) item.cancel()
    this.submissions.clear()
    for (const sessionId of [...this.sessions.keys()]) this.disposeSession(sessionId)
    for (const timer of this.pendingDesktopTimers.values()) clearTimeout(timer)
    this.pendingDesktopTimers.clear()
    this.pendingDesktopEvents.clear()
    this.desktopEventSignatures.clear()
    this.desktopWaitKeys.clear()
    this.terminalSyncClaims.clear()
    this.writers.clear()
    this.bindings.clear()
  }

  /** Cancel and release sessions even when the owning ChatView is no longer mounted. */
  async closeTabSessions(tabPath: string, cancelExecution: boolean) {
    // Closing a tab must also cancel requests that have not created an FP session yet.
    for (const item of [...this.submissions.values()]) {
      if (item.binding.tabPath === tabPath) item.cancel()
    }
    const targets = [...this.bindings.entries()]
      .filter(([, binding]) => binding.tabPath === tabPath)
    for (const [sessionId] of targets) {
      const managed = this.sessions.get(sessionId)
      if (!managed) {
        this.bindings.delete(sessionId)
        continue
      }
      if (cancelExecution && !isTerminalSession(managed.controller)) {
        const state = managed.controller.state
        try {
          if (state.isPreviewing || state.isPlanning || !state.currentTreeId) {
            await Promise.resolve(managed.controller.cancelPlan())
          } else {
            await Promise.resolve(managed.controller.cancelExecution())
          }
        } catch (error) {
          console.warn('[FastPlan] cancel session while closing tab failed', error)
        }
      }
      const state = managed.controller.state
      const idle = !state.isPlanning && !state.plannedTree && !state.isPreviewing && !state.isPaused
      if (cancelExecution || idle || isTerminalSession(managed.controller)) this.disposeSession(sessionId)
    }
    if (cancelExecution) useExecutionActivityStore().removeByPath(tabPath)
  }

  /**
   * Mirror FastPlan process events through one ordered, coalesced IPC stream per
   * desktop conversation. This survives ChatView activation changes.
   */
  emitDesktopEvent(conversationId: string, event: Record<string, unknown>) {
    const sid = conversationId.trim()
    if (!sid || !event || typeof event !== 'object') return Promise.resolve()
    const key = this.desktopEventKey(event)
    const signature = JSON.stringify(event)
    const signatureKey = `${sid}:${key}`
    const previous = this.desktopEventSignatures.get(signatureKey)
    const now = Date.now()
    if (previous?.signature === signature && now - previous.at < 500) return Promise.resolve()
    this.desktopEventSignatures.set(signatureKey, { signature, at: now })

    const type = String(event.type || event.kind || '')
    if (!COALESCED_DESKTOP_EVENTS.has(type)) {
      return this.invokeDesktopEvents(sid, [...this.takePendingDesktopEvents(sid), event])
    }

    const pending = this.pendingDesktopEvents.get(sid) || new Map<string, Record<string, unknown>>()
    pending.set(key, event)
    this.pendingDesktopEvents.set(sid, pending)
    if (!this.pendingDesktopTimers.has(sid)) {
      this.pendingDesktopTimers.set(sid, setTimeout(() => {
        void this.invokeDesktopEvents(sid, this.takePendingDesktopEvents(sid))
      }, 100))
    }
    return Promise.resolve()
  }

  acquireWriter(conversationId: string, writerId: string) {
    const owner = this.writers.get(conversationId)
    if (owner && owner !== writerId) return false
    this.writers.set(conversationId, writerId)
    return true
  }

  claimTerminalSync(sessionId: string, completionId: string) {
    const key = `${sessionId}:${completionId || 'terminal'}`
    if (this.terminalSyncClaims.has(key)) return false
    this.terminalSyncClaims.add(key)
    return true
  }

  releaseTerminalSync(sessionId: string, completionId: string) {
    this.terminalSyncClaims.delete(`${sessionId}:${completionId || 'terminal'}`)
  }

  releaseWriter(conversationId: string, writerId: string) {
    if (this.writers.get(conversationId) === writerId) this.writers.delete(conversationId)
  }

  /** Test/diagnostic surface; does not expose mutable registry entries. */
  stats() {
    let active = 0
    let retained = 0
    for (const session of this.sessions.values()) {
      if (!isTerminalSession(session.controller)) active += 1
      if (session.owners.size) retained += 1
    }
    return { total: this.sessions.size, active, retained }
  }

  private pruneReleasedSessions() {
    const released = [...this.sessions.entries()]
      .filter(([, value]) => value.owners.size === 0 && isTerminalSession(value.controller))
      .sort((a, b) => a[1].lastAccessedAt - b[1].lastAccessedAt)
    while (released.length > MAX_RELEASED_SESSIONS) {
      const candidate = released.shift()
      if (candidate) this.disposeSession(candidate[0])
    }
  }

  private syncExecutionActivity(sessionId: string, state: FastPlanExecutionController['state']) {
    const binding = this.bindings.get(sessionId)
    if (!binding) return
    // Observe intervening running -> paused transitions even while no UI requests a target.
    this.controlTarget(binding.conversationId, binding.entryId)
    if (String(state.connectionStatus || '').toLowerCase() === 'connected') {
      chatSwitchPerf.markConnectionReady(binding.tabPath)
    }
    const phase = this.hasSubmission(binding.conversationId, binding.entryId) ? 'submitting' : executionPhase(state)
    const store = useExecutionActivityStore()
    if (['idle', 'completed', 'failed', 'cancelled'].includes(phase)) {
      store.remove(binding.conversationId, 'fastplan')
      return
    }
    store.update(binding.conversationId, binding.tabPath, 'fastplan', {
      phase,
      protected: true,
      sessionId,
      entryId: binding.entryId,
      executionMode: state.executionMode || binding.executionMode,
      connectionStatus: state.connectionStatus,
    })
  }

  private async syncTerminalState(
    sessionId: string,
    name: string,
    payload: any,
    state: FastPlanExecutionController['state'],
  ) {
    const binding = this.bindings.get(sessionId)
    const submission = binding && this.submissions.get(`${binding.tabPath}:${binding.entryId}`)
    if (submission && !submission.accepted) return
    const status = terminalStatus(name, payload, state)
    if (!binding || !status) return
    const event = payload?.event && typeof payload.event === 'object'
      ? payload.event as Record<string, unknown>
      : payload && typeof payload === 'object'
        ? payload as Record<string, unknown>
        : undefined
    const completionId = String(
      event?.tree_id
      || event?.treeId
      || state.currentTreeId
      || (state as any).activeUserMessageId
      || binding.entryId,
    )
    if (!this.claimTerminalSync(sessionId, completionId)) return
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const args = {
        conversationId: binding.conversationId,
        content: extractFinalAnswer(state, event),
        groupId: binding.groupId || null,
        modelId: binding.modelId || null,
        planRuntime: buildPlanRuntime(state),
        fastplanSessionId: sessionId,
        entryId: binding.entryId,
        completionId,
        terminalStatus: status,
      }
      let result: { extra_config?: string } | undefined
      let lastError: unknown
      for (let attempt = 0; attempt < 3; attempt += 1) {
        try {
          result = await invoke<{ extra_config?: string }>('sync_desktop_plan_assistant_message', args)
          lastError = undefined
          break
        } catch (error) {
          lastError = error
          if (attempt < 2) await new Promise(resolve => setTimeout(resolve, 200 * (attempt + 1)))
        }
      }
      if (lastError) throw lastError
      if (typeof result?.extra_config === 'string') {
        const { patchSharedConversationExtraConfig } = await import('@/stores/chat')
        patchSharedConversationExtraConfig(binding.conversationId, result.extra_config)
      }
    } catch (error) {
      console.warn('[FastPlan] persist terminal result failed', error)
      this.releaseTerminalSync(sessionId, completionId)
    }
  }

  private mirrorControllerState(
    sessionId: string,
    payload: any,
    state: FastPlanExecutionController['state'],
  ) {
    const binding = this.bindings.get(sessionId)
    if (!binding) return
    const event = payload?.event
    if (event && typeof event === 'object') {
      void this.emitDesktopEvent(binding.conversationId, event as Record<string, unknown>)
    }
    const next = this.desktopWaitKey(state)
    const previous = this.desktopWaitKeys.get(binding.conversationId) || ''
    if (next === previous) return
    this.desktopWaitKeys.set(binding.conversationId, next)
    const mode = state.executionMode || binding.executionMode || 'auto'
    if (next.startsWith('preview:')) {
      void this.emitDesktopEvent(binding.conversationId, {
        type: 'plan_awaiting_confirmation',
        tree_id: state.currentTreeId,
        execution_mode: mode,
      })
    } else if (next.startsWith('step:')) {
      void this.emitDesktopEvent(binding.conversationId, {
        type: 'tree_paused',
        tree_id: state.currentTreeId,
        node_id: state.lastCompletedNodeId || state.stepConfirmNodeId,
        execution_mode: mode,
      })
    } else if (previous.startsWith('preview:')) {
      const cancelled = String(state.executionStatus || '').toLowerCase() === 'cancelled' || !!state.planningError
      void this.emitDesktopEvent(binding.conversationId, {
        type: cancelled || !state.isPlanning ? 'plan_cancelled' : 'plan_confirmed',
      })
    } else if (previous.startsWith('step:')) {
      const cancelled = String(state.executionStatus || '').toLowerCase() === 'cancelled'
      void this.emitDesktopEvent(binding.conversationId, {
        type: cancelled ? 'plan_cancelled' : 'tree_resumed',
      })
    }
  }

  private desktopWaitKey(state: FastPlanExecutionController['state']) {
    if (state.treeCompleted || String(state.executionStatus || '').toLowerCase() === 'cancelled') return ''
    if (state.isPreviewing) return `preview:${state.currentTreeId || ''}`
    const nodeId = state.lastCompletedNodeId || state.stepConfirmNodeId
    if (state.isPaused && nodeId) return `step:${nodeId}`
    return ''
  }

  private desktopEventKey(event: Record<string, unknown>) {
    const type = String(event.type || event.kind || 'unknown')
    return [type, event.tree_id || event.treeId || '', event.node_id || event.nodeId || ''].join(':')
  }

  private takePendingDesktopEvents(conversationId: string) {
    const timer = this.pendingDesktopTimers.get(conversationId)
    if (timer) clearTimeout(timer)
    this.pendingDesktopTimers.delete(conversationId)
    const pending = this.pendingDesktopEvents.get(conversationId)
    this.pendingDesktopEvents.delete(conversationId)
    return pending ? [...pending.values()] : []
  }

  private invokeDesktopEvents(conversationId: string, events: Record<string, unknown>[]) {
    if (!events.length) return Promise.resolve()
    const previous = this.desktopEventChains.get(conversationId) || Promise.resolve()
    const current = previous.catch(() => undefined).then(async () => {
      // 桌面规划事件不再镜像到 App（远程控制 / FastCI 中继已移除），此处保留空实现
      void conversationId
      void events
    })
    this.desktopEventChains.set(conversationId, current)
    void current.finally(() => {
      if (this.desktopEventChains.get(conversationId) === current) {
        this.desktopEventChains.delete(conversationId)
      }
    })
    return current
  }
}

export const fastPlanSessionManager = new FastPlanSessionManager()

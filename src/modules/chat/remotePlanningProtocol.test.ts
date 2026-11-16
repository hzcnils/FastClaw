import { afterEach, describe, expect, it } from 'vitest'
import type { Message } from '@/types'
import { applyRemotePlanEventToMessages, clearLivePlanState, hydrateMessagePlanRuntime,
  normalizePlanRuntime, planConfirmationFromRuntime, remotePlanLiveBySession } from '@/integrations/chat-plan/remotePlanRuntime'

const sid = 'protocol-session'
afterEach(() => clearLivePlanState(sid))
const runtime = () => ({ controlProtocol: 1, controlWaitId: 'wait-1', isPreviewing: true,
  plannedTree: { id: 'tree-1' }, currentTreeId: 'tree-1', executionMode: 'manual' })
function fixture() {
  const messages = [{ id: 'question', conversation_id: sid, role: 'user', content: 'question' }] as Message[]
  const apply = (event: Record<string, unknown>) => applyRemotePlanEventToMessages(messages, sid, event)
  apply({ type: 'snapshot', planRuntime: runtime() })
  return { messages, apply, live: () => remotePlanLiveBySession[sid] }
}

describe('remote wait protocol projection', () => {
  it('normalizes camel/snake wait metadata and hides a versioned wait until its slot exists', () => {
    const normalized = normalizePlanRuntime({ control_protocol: 1, control_wait_id: 'wait', is_previewing: true })!
    expect(normalized.controlWaitId).toBe('wait'); expect(planConfirmationFromRuntime(normalized)).not.toBeNull()
    expect(planConfirmationFromRuntime({ ...normalized, controlWaitId: null })).toBeNull()
    expect(planConfirmationFromRuntime({ ...normalized, controlProtocol: 0, controlWaitId: null })).not.toBeNull()
  })
  it('retains a published token through progress and repeated wait events', () => {
    const f = fixture()
    for (const event of [{ type: 'thinking', content: 'more' }, { type: 'plan_awaiting_confirmation' }]) {
      f.apply(event)
      expect(f.live().controlWaitId).toBe('wait-1'); expect(f.live().controlProtocol).toBe(1)
    }
  })
  it.each(['planning_start', 'plan_confirmed', 'tree_resumed', 'plan_cancelled', 'tree_completed', 'error'])(
    'removes a previous wait token after %s', type => {
      const f = fixture(); f.apply({ type, controlProtocol: 1 })
      expect(f.live().controlWaitId).toBeNull(); expect(planConfirmationFromRuntime(f.live())).toBeNull()
    })
  it('replaces the same-node step identity only when a new authoritative snapshot arrives', () => {
    const f = fixture()
    const step = { ...runtime(), isPreviewing: false, isPaused: true, executionMode: 'step', lastCompletedNodeId: 'node' }
    f.apply({ type: 'snapshot', planRuntime: step })
    f.apply({ type: 'tree_paused', tree_id: 'tree-1', node_id: 'node' })
    expect(f.live().controlWaitId).toBe('wait-1')
    f.apply({ type: 'tree_resumed' }); expect(planConfirmationFromRuntime(f.live())).toBeNull()
    f.apply({ type: 'tree_paused', tree_id: 'tree-1', node_id: 'node' }); expect(planConfirmationFromRuntime(f.live())).toBeNull()
    f.apply({ type: 'snapshot', planRuntime: { ...step, controlWaitId: 'wait-2' } })
    expect(f.live().controlWaitId).toBe('wait-2'); expect(planConfirmationFromRuntime(f.live())?.kind).toBe('step')
  })
  it('preserves metadata in hydrated snapshots and clears it with the session', () => {
    const f = fixture(), message = { id: 'assistant', role: 'assistant' } as Message
    hydrateMessagePlanRuntime(message, { planRuntime: runtime() })
    expect(message.planRuntime?.controlWaitId).toBe('wait-1')
    clearLivePlanState(sid); expect(remotePlanLiveBySession[sid]).toBeUndefined()
    f.apply({ type: 'planning_start', controlProtocol: 1 }); expect(f.live().controlWaitId).toBeNull()
  })
})

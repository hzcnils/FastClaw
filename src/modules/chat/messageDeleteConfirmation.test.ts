// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h, KeepAlive, reactive, ref } from 'vue'
import { mount } from '@vue/test-utils'
import type { Message } from '@/types'
import source from '@/components/chat/MessageBubble.vue?raw'
import { useMessageDeleteConfirmation } from './messageDeleteConfirmation'
const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function fixture() {
  const state = reactive({ message: { id: 'm', conversation_id: 'a', content: 'body' } as Message }), remove = vi.fn(), active = ref(true)
  let actions!: ReturnType<typeof useMessageDeleteConfirmation>
  const Child = defineComponent({ setup() { actions = useMessageDeleteConfirmation(() => state.message, remove); return () => h('button') } })
  const wrapper = mount(defineComponent({ setup: () => () => h(KeepAlive, () => active.value ? h(Child) : null) }))
  cleanup.push(() => wrapper.unmount())
  return { state, remove, active, wrapper, get actions() { return actions } }
}
describe('delete confirmation target', () => {
  it('confirms the opening target only once', () => {
    const f = fixture(); f.actions.open(); f.actions.confirm(); f.actions.confirm()
    expect(f.remove).toHaveBeenCalledExactlyOnceWith('m')
    expect(source).toContain('@confirm="deleteConfirmation.confirm"'); expect(source).toContain('@click="deleteConfirmation.open"')
  })
  it.each(['message','content-revert','cancel','unmount'])('rejects a stale confirmation on %s', change => {
    const f = fixture(); f.actions.open()
    if (change === 'message') f.state.message = { ...f.state.message, id: 'new' }
    if (change === 'content-revert') { f.state.message.content = 'new'; f.state.message.content = 'body' }
    if (change === 'cancel') f.actions.cancel()
    if (change === 'unmount') { f.wrapper.unmount(); cleanup.pop() }
    f.actions.confirm(); expect(f.remove).not.toHaveBeenCalled()
  })
  it('deactivation and reactivation do not revive the old confirmation', async () => {
    const f = fixture(); f.actions.open(); f.active.value = false; await f.wrapper.vm.$nextTick()
    f.active.value = true; await f.wrapper.vm.$nextTick(); f.actions.confirm(); expect(f.remove).not.toHaveBeenCalled()
    f.actions.open(); f.actions.confirm(); expect(f.remove).toHaveBeenCalledOnce()
  })
})

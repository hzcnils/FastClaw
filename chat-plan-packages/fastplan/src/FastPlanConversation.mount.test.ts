// @vitest-environment jsdom
import { defineComponent, nextTick, markRaw } from 'vue'
import { createFastPlanExecutionController } from 'planflow-runtime'
import { flushPromises, mount } from '@vue/test-utils'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import FastPlanConversation from './FastPlanConversation.vue'

const tree = { id: 'tree-1', root: { children: [{ id: 'phase-1', name: 'Phase', children: [
  { id: 'node-1', name: 'Collect', node_type: { type: 'LlmTask', config: { prompt_template: 'Collect sources' } } },
] }] }, metadata: {} }

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'scrollTo', { configurable: true, value: vi.fn() })
})

describe('FastPlanConversation embedded confirmation', () => {
  it.each(['rejected', 'failed'] as const)('preserves the confirmation bar after a %s host control', async outcome => {
    const onControl = outcome === 'rejected' ? vi.fn().mockResolvedValue(false) : vi.fn().mockRejectedValue(new Error('offline'))
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 's', autoLoad: false, showComposer: false, onControl,
      interactionPlacement: 'inline', executionMode: 'manual', confirmationPolicy: 'strict',
    } })
    try {
      Object.assign((wrapper.vm as any).runtime.state, { plannedTree: tree, currentTreeId: tree.id, isPreviewing: true })
      await nextTick()
      const confirm = wrapper.findAll('.jcr-plan-confirm__options button').find(button => button.text().includes('确认并执行'))!
      await confirm.trigger('click')
      await flushPromises()
      expect(onControl).toHaveBeenCalledWith(expect.objectContaining({ type: 'confirm', tree }))
      expect(wrapper.find('.fastplan-conversation__inline-interaction').text()).toContain('确认并执行')
      expect(wrapper.emitted('error')?.length || 0).toBe(outcome === 'failed' ? 1 : 0)
    } finally { wrapper.unmount() }
  })

  it('routes exposed controls to the host without invoking the local controller', async () => {
    const onControl = vi.fn().mockResolvedValue(true)
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false, onControl } })
    const vm = wrapper.vm as any
    const local = vi.spyOn(vm.runtime, 'confirmPlan')
    const attachments = [{ path: '/input.pdf' }]
    await vm.confirmPlan(tree, attachments)
    await vm.pauseExecution(); await vm.resumeExecution(); await vm.retryNode('n'); await vm.skipNode('n')
    await vm.respondHumanCheckpoint({ action: 'submit', values: { city: '西安' } })
    await vm.updateNodeOutput('n', { content: 'edited' }); await vm.updateNextNodeConfig('n', { prompt_template: 'edited' })
    await vm.cancelPlan(); await vm.cancelExecution()
    expect(onControl.mock.calls.map(([command]) => command.type)).toEqual([
      'confirm', 'pause', 'resume', 'retry-node', 'skip-node', 'human-response', 'edit-node-output', 'edit-node-config', 'cancel-plan', 'cancel-execution',
    ])
    expect(onControl).toHaveBeenCalledWith({ type: 'confirm', tree, attachments })
    expect(local).not.toHaveBeenCalled()
    wrapper.unmount()
    expect(vm.resumeExecution()).toBe(false)
    expect(vm.updateNodeOutput('n', 'late')).toBe(false)
    expect(vm.updateNextNodeConfig('n', { prompt_template: 'late' })).toBe(false)
    expect(onControl).toHaveBeenCalledTimes(10)
  })

  it('keeps standalone node edits and template return values, but blocks new calls after unmount', async () => {
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false } })
    const vm = wrapper.vm as any
    const output = vi.spyOn(vm.runtime, 'updateNodeOutput').mockReturnValue(true)
    const config = vi.spyOn(vm.runtime, 'updateNextNodeConfig').mockReturnValue(true)
    const execute = vi.spyOn(vm.runtime, 'executeTemplate').mockResolvedValue(tree)
    const save = vi.spyOn(vm.runtime, 'saveAsTemplate').mockResolvedValue({ id: 'template' })
    expect(vm.updateNodeOutput('n', 'value')).toBe(true)
    expect(vm.updateNextNodeConfig('n', { prompt: 'value' })).toBe(true)
    await expect(vm.executeTemplate(tree, [], { preview: true })).resolves.toBe(tree)
    await expect(vm.saveAsTemplate('name', 'desc', tree)).resolves.toEqual({ id: 'template' })
    expect(output).toHaveBeenCalledExactlyOnceWith('n', 'value')
    expect(config).toHaveBeenCalledExactlyOnceWith('n', { prompt: 'value' })
    wrapper.unmount()
    await expect(vm.executeTemplate(tree)).resolves.toBe(false)
    await expect(vm.saveAsTemplate('late')).resolves.toBe(false)
    expect(execute).toHaveBeenCalledTimes(1); expect(save).toHaveBeenCalledTimes(1)
  })

  it('allows an already accepted template save to finish after the managed view is unmounted', async () => {
    let resolve!: (value: any) => void
    const runtime = markRaw(createFastPlanExecutionController({ sessionId: 's', fetch: vi.fn(async () => new Response('{}')) }))
    Object.assign(runtime.state, { plannedTree: structuredClone(tree), canSaveTemplate: true })
    const save = vi.spyOn(runtime.client, 'createTemplate').mockReturnValue(new Promise(done => { resolve = done }))
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false, runtime } })
    const task = (wrapper.vm as any).saveAsTemplate('name')
    wrapper.unmount()
    expect(save.mock.calls[0][1]?.aborted).toBe(false)
    resolve({ id: 'saved' })
    await expect(task).resolves.toEqual({ id: 'saved' })
    expect(runtime.state.canSaveTemplate).toBe(false)
    runtime.disconnect()
  })

  it('keeps the standalone controller fallback when no dispatcher is supplied', async () => {
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false } })
    const vm = wrapper.vm as any
    const confirm = vi.spyOn(vm.runtime, 'confirmPlan').mockResolvedValue(true)
    const pause = vi.spyOn(vm.runtime, 'pauseExecution').mockReturnValue(true)
    const cancel = vi.spyOn(vm.runtime, 'cancelPlan').mockReturnValue(true)
    await vm.confirmPlan(tree, [{ path: '/input.pdf' }])
    expect(confirm).toHaveBeenCalledExactlyOnceWith(tree, [{ path: '/input.pdf' }])
    expect(vm.pauseExecution()).toBe(true)
    expect(vm.cancelPlan()).toBe(true)
    expect(pause).toHaveBeenCalledTimes(1)
    expect(cancel).toHaveBeenCalledTimes(1)
    wrapper.unmount()
  })

  it('does not let a late confirmation hide a newer turn confirmation', async () => {
    let resolve!: (accepted: boolean) => void
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 's', autoLoad: false, showComposer: false,
      onControl: () => new Promise<boolean>(done => { resolve = done }),
      interactionPlacement: 'inline', executionMode: 'manual', confirmationPolicy: 'strict',
    } })
    const vm = wrapper.vm as any
    Object.assign(vm.runtime.state, { plannedTree: tree, currentTreeId: tree.id, isPreviewing: true })
    await nextTick()
    const pending = vm.confirmPlan()
    vm.runtime.state.plannedTree = { ...tree, id: 'tree-2' }
    vm.runtime.state.activeUserMessageId = 'turn-2'
    await nextTick()
    resolve(true); await pending; await nextTick()
    expect(wrapper.find('.fastplan-conversation__inline-interaction').text()).toContain('确认并执行')
    wrapper.unmount()
  })

  it.each(['allow', 'deny', 'unmount'] as const)('keeps the internal provider gate view-owned: %s', async outcome => {
    let resolve!: (value: boolean) => void
    const beforeSend = vi.fn(() => new Promise<boolean>(done => { resolve = done }))
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false, beforeSend, model: { id: 'model' } } })
    const send = vi.spyOn((wrapper.vm as any).runtime, 'sendChat').mockResolvedValue(true)
    const task = (wrapper.vm as any).sendChat('question')
    expect(send).not.toHaveBeenCalled()
    expect(beforeSend).toHaveBeenCalledWith({ id: 'model' }, 'question')
    if (outcome === 'unmount') wrapper.unmount()
    resolve(outcome !== 'deny')
    await expect(task).resolves.toBe(outcome === 'allow')
    expect(send).toHaveBeenCalledTimes(outcome === 'allow' ? 1 : 0)
    if (outcome !== 'unmount') wrapper.unmount()
  })
  it('keeps completed-turn process Markdown collapsed until requested', async () => {
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false, showComposer: false } })
    try {
      const runtime = (wrapper.vm as any).runtime
      runtime.state.messages = [
        { id: 'u1', role: 'user', content: 'prepare report' },
        { id: 'p1', role: 'assistant', content: '## Process\n- collected sources' },
        { id: 'f1', role: 'assistant', content: '# Final\ncomplete report', contentKind: 'final_answer', isFinalAnswer: true },
      ]
      await nextTick()

      const summary = wrapper.get('.fastplan-conversation__process-summary')
      expect(summary.text()).toContain('Process collected sources')
      expect(wrapper.find('.fastplan-conversation__process-detail').exists()).toBe(false)

      await summary.trigger('click')
      await nextTick()
      const detail = wrapper.get('.fastplan-conversation__process-detail')
      expect(detail.text()).toContain('Process')

      await detail.get('button').trigger('click')
      await nextTick()
      expect(wrapper.find('.fastplan-conversation__process-detail').exists()).toBe(false)
      expect(wrapper.find('.fastplan-conversation__process-summary').exists()).toBe(true)
    } finally {
      wrapper.unmount()
    }
  })

  it('restores a failed turn and a successful turn as two full planning panels', async () => {
    const record = (id: number, treeId: string, state: any, status: string) => ({ id, tree_id: treeId,
      tree_json: { ...tree, id: treeId }, state_json: state, status, created_at: `2026-09-04T01:00:0${id}Z` })
    const data = { messages: [
      { id: 25, role: 'user', content: 'first', metadata: JSON.stringify({ turn_id: 'turn-a' }) },
      { id: 26, role: 'user', content: 'first', metadata: '{}' },
      { id: 27, role: 'assistant', content: 'invalid_api_key', metadata: JSON.stringify({ msg_type: 'error', tree_id: 'a',
        process_trace: { user_message_id: 25, status: 'failed', turn_id: 'turn-a' } }) },
      { id: 54, role: 'user', content: 'second', metadata: JSON.stringify({ turn_id: 'turn-b' }) },
      { id: 56, role: 'assistant', content: 'second result', metadata: JSON.stringify({ tree_id: 'b',
        process_trace: { user_message_id: 54, status: 'completed', turn_id: 'turn-b' } }) },
    ], executions: [
      record(1, 'a', { __processTrace: { user_message_id: 25, status: 'failed' } }, 'failed'),
      record(2, 'a', { 'node-1': { status: 'failed', error: 'invalid_api_key' }, __runtime: { anchorMessageId: 'temporary-a' } }, 'failed'),
      record(3, 'b', { 'node-1': { status: 'completed', output: 'second output' }, __runtime: { anchorMessageId: 'temporary-b' },
        __finalResult: { content: 'second result' }, __resultFiles: [{ name: 'second.txt', url: '/second.txt' }] }, 'completed'),
    ] }
    vi.stubGlobal('fetch', vi.fn(async (url: any) => new Response(JSON.stringify(String(url).includes('execution-state') ? { has_state: false } : data))))
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false, showComposer: false } })
    try {
      await (wrapper.vm as any).runtime.loadSession('s', { connect: false })
      await nextTick()
      expect(wrapper.findAll('[data-planning-turn]')).toHaveLength(2)
      expect(wrapper.find('[data-planning-turn="25"]').exists()).toBe(true)
      expect(wrapper.find('[data-planning-turn="54"]').exists()).toBe(true)
      expect(wrapper.findAll('.fpp-ui[data-planning-turn]')).toHaveLength(2)
      expect(wrapper.text()).toContain('invalid_api_key')
      expect(wrapper.find('a[href="/second.txt"]').exists()).toBe(true)
    } finally { wrapper.unmount(); vi.unstubAllGlobals() }
  })

  it('keeps both turn panels, artifacts and view selections isolated as the next turn starts', async () => {
    const Panel = defineComponent({ name: 'FastPlanPlanningPanel', props: ['state', 'activeView', 'phase'],
      emits: ['view-change', 'node-select', 'confirm'], template: '<div class="test-plan">{{ state.lastPrompt }}</div>' })
    const wrapper = mount(FastPlanConversation, { props: { sessionId: 's', autoLoad: false, showComposer: false },
      global: { stubs: { FastPlanPlanningPanel: Panel } } })
    const runtime = (wrapper.vm as any).runtime
    runtime.state.messages = [{ id: 'u1', role: 'user', content: 'first' }, { id: 'a1', role: 'assistant', content: 'answer' }]
    Object.assign(runtime.state, { activeUserMessageId: 'u1', currentTreeId: 'tree-1', plannedTree: structuredClone(tree),
      lastPrompt: 'first', treeCompleted: true, executionStatus: 'completed',
      executionState: { 'node-1': { status: 'completed', output: 'first output' } },
      finalResult: { content: 'first answer' }, resultFiles: [{ name: 'first.txt', url: '/first.txt' }] })
    await nextTick()
    const original = wrapper.findComponent(Panel)
    original.vm.$emit('view-change', 'dag')
    await nextTick()
    expect(original.props('activeView')).toBe('dag')
    const snapshot = JSON.parse(JSON.stringify({ ...runtime.state, messages: undefined, executionHistory: undefined }))
    runtime.state.executionHistory = [{ ...snapshot, treeId: 'tree-1', anchorMessageId: 'u1' }]
    runtime.state.messages.push({ id: 'u2', role: 'user', content: 'second' })
    Object.assign(runtime.state, { activeUserMessageId: 'u2', currentTreeId: '', plannedTree: null,
      lastPrompt: 'second', isPlanning: true, treeCompleted: false, finalResult: null, resultFiles: [], executionState: {} })
    await nextTick()
    const panels = wrapper.findAllComponents(Panel)
    expect(panels).toHaveLength(2)
    expect(panels[0].vm).toBe(original.vm)
    expect(panels[0].props('state').executionState['node-1'].output).toBe('first output')
    expect(panels[0].props('activeView')).toBe('dag')
    expect(panels[1].props('activeView')).toBe('list')
    expect(wrapper.find('a[href="/first.txt"]').exists()).toBe(true)
    panels[0].vm.$emit('node-select', { id: 'node-1' })
    expect(wrapper.emitted('node-select')?.[0]?.[0]).toMatchObject({ treeId: 'tree-1', anchorMessageId: 'u1',
      state: { lastPrompt: 'first' } })
    const confirm = vi.spyOn(runtime, 'confirmPlan')
    panels[0].vm.$emit('confirm', tree)
    expect(confirm).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('shows the internal composer by default in standalone layout', () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false,
    } })
    expect(wrapper.find('.jcr-conversation__footer').exists()).toBe(true)
    expect(wrapper.find('.jcr-composer').exists()).toBe(true)
    wrapper.unmount()
  })

  it('forwards host-controlled file selection and attachment actions', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false,
      composerActions: [{ id: 'upload', label: 'Upload', kind: 'file', multiple: true }],
      composerAttachments: [{ id: 'failed', kind: 'file', name: 'failed.pdf', status: 'error', error: 'failed' }],
    } })
    const input = wrapper.get('input[type="file"]')
    const file = new File(['report'], 'report.pdf', { type: 'application/pdf' })
    Object.defineProperty(input.element, 'files', { configurable: true, value: [file] })
    await input.trigger('change')
    expect(wrapper.emitted('composer-files')?.[0]?.[0]).toMatchObject({ action: 'upload', files: [file] })

    await wrapper.get('.jcr-composer-attachment__retry').trigger('click')
    expect(wrapper.emitted('attachment-action')?.[0]?.[0]).toMatchObject({ action: 'retry', attachment: { id: 'failed' } })
    wrapper.unmount()
  })

  it('submits sanitized completed attachments supplied by the host', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false,
      composerAttachments: [{ id: 'ready', kind: 'file', name: 'report.pdf', path: 'D:/uploads/report.pdf', status: 'complete' }],
    } })
    const runtime = (wrapper.vm as any).runtime
    const send = vi.spyOn(runtime, 'sendChat').mockReturnValue(true)
    await wrapper.get('textarea').setValue('analyze this')
    await wrapper.get('form').trigger('submit')
    expect(send).toHaveBeenCalledWith('analyze this', { model: undefined, attachments: [
      { filename: 'report.pdf', abs_path: 'D:/uploads/report.pdf', ext: 'pdf' },
    ] })
    wrapper.unmount()
  })

  it('can hide an obsolete task list without changing its runtime state', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: false,
      showTaskList: false, interactionPlacement: 'inline',
      executionMode: 'manual', confirmationPolicy: 'strict',
    } })
    await (wrapper.vm as any).runtime.processEvent({ type: 'plan_generated', tree })
    await nextTick()
    expect((wrapper.vm as any).runtime.state.plannedTree).toBeTruthy()
    expect(wrapper.find('.fastplan-conversation__inline-interaction').exists()).toBe(false)
    wrapper.unmount()
  })

  it('moves the task list to a host target mounted after the conversation', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: true,
      composerMode: 'host', composerTarget: null,
      executionMode: 'manual', confirmationPolicy: 'strict',
    } })
    await (wrapper.vm as any).runtime.processEvent({ type: 'plan_generated', tree })
    await nextTick()
    expect(wrapper.find('[data-composer-fallback="inline"]').exists()).toBe(true)

    const target = document.createElement('div')
    document.body.appendChild(target)
    await wrapper.setProps({ composerTarget: target })
    await nextTick()
    expect(target.querySelector('.fastplan-conversation__host-composer-contribution')).toBeTruthy()
    expect(wrapper.find('.fastplan-conversation__host-composer-contribution').exists()).toBe(false)
    wrapper.unmount()
    target.remove()
  })

  it('contributes the task list to a host-owned chat-renderer composer', async () => {
    const target = document.createElement('div')
    document.body.appendChild(target)
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: true,
      composerMode: 'host', composerTarget: target,
      density: 'compact',
      executionMode: 'manual', confirmationPolicy: 'strict',
    } })
    await (wrapper.vm as any).runtime.processEvent({ type: 'plan_generated', tree })
    await nextTick()
    expect(wrapper.find('.jcr-composer').exists()).toBe(false)
    expect(target.querySelector('.fastplan-conversation__host-composer-contribution')).toBeTruthy()
    expect(target.querySelector('.fastplan-conversation__host-composer-contribution')?.getAttribute('data-density')).toBe('compact')
    expect(target.textContent).toContain('确认并执行')
    wrapper.unmount()
    target.remove()
  })

  it('mounts manual plan confirmation in the inline host region', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: false,
      interactionPlacement: 'inline', executionMode: 'manual', confirmationPolicy: 'strict',
    } })
    await (wrapper.vm as any).runtime.processEvent({ type: 'plan_generated', tree })
    await nextTick()
    const interaction = wrapper.find('.fastplan-conversation__inline-interaction')
    expect(interaction.exists()).toBe(true)
    expect(interaction.text()).toContain('确认并执行')
    wrapper.unmount()
  })

  it('removes the redundant edit option after entering plan editing', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: false,
      interactionPlacement: 'inline', executionMode: 'manual', confirmationPolicy: 'strict',
    } })
    await (wrapper.vm as any).runtime.processEvent({ type: 'plan_generated', tree })
    await nextTick()

    const interaction = wrapper.find('.fastplan-conversation__inline-interaction')
    const edit = interaction.findAll('.jcr-plan-confirm__options button')
      .find(button => button.text().includes('修改规划'))
    expect(edit).toBeTruthy()
    await edit!.trigger('click')
    await nextTick()

    const labels = interaction.findAll('.jcr-plan-confirm__options button').map(button => button.text())
    expect(labels).toHaveLength(2)
    expect(labels[0]).toContain('确认修改并执行')
    expect(labels[1]).toContain('取消任务')
    expect(interaction.text()).not.toContain('继续修改规划')
    wrapper.unmount()
  })

  it('mounts a continue action after a strict step completes', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: false,
      interactionPlacement: 'inline', executionMode: 'step', confirmationPolicy: 'strict',
    } })
    const runtime = (wrapper.vm as any).runtime
    runtime.state.plannedTree = structuredClone(tree)
    runtime.state.currentTreeId = 'tree-1'
    runtime.state.isPaused = true
    runtime.state.lastCompletedNodeId = 'node-1'
    await nextTick()
    expect(wrapper.find('.fastplan-conversation__inline-interaction').text()).toContain('继续下一步')
    wrapper.unmount()
  })

  it('mounts a continue action for legacy step consumers', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: false,
      interactionPlacement: 'inline', executionMode: 'step',
    } })
    const runtime = (wrapper.vm as any).runtime
    runtime.state.plannedTree = structuredClone(tree)
    runtime.state.currentTreeId = 'tree-1'
    runtime.state.isPaused = true
    runtime.state.lastCompletedNodeId = 'node-1'
    await nextTick()
    expect(wrapper.find('.fastplan-conversation__inline-interaction').text()).toContain('继续下一步')
    wrapper.unmount()
  })

  it('renders errors in the opt-in inline host region without changing the default', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: false,
      interactionPlacement: 'inline', errorPlacement: 'inline',
    } })
    ;(wrapper.vm as any).runtime.state.error = new Error('connection failed')
    await nextTick()
    const error = wrapper.find('.fastplan-conversation__error--inline')
    expect(error.exists()).toBe(true)
    expect(error.text()).toContain('connection failed')
    wrapper.unmount()
  })

  it('renders protocol failures once inside the assistant message below its model header', async () => {
    const wrapper = mount(FastPlanConversation, { props: {
      sessionId: 'session-1', autoLoad: false, showComposer: false,
      interactionPlacement: 'inline', errorPlacement: 'inline',
      model: { model: 'qwen', name: 'Qwen' },
    } })
    await (wrapper.vm as any).runtime.processEvent({ type: 'error', message: 'planning failed' })
    await nextTick()

    const header = wrapper.find('.jcr-message__header')
    const messageError = wrapper.find('.jcr-message-error')
    expect(header.exists()).toBe(true)
    expect(header.text()).toContain('Qwen')
    expect(messageError.text()).toContain('planning failed')
    expect(wrapper.find('.fastplan-conversation__error--inline').exists()).toBe(false)
    expect(header.element.compareDocumentPosition(messageError.element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    wrapper.unmount()
  })
})

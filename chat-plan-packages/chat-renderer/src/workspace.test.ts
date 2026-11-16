// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { createSSRApp, defineComponent, h, onMounted } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { describe, expect, it, vi } from 'vitest'
import ChatAuxiliaryPanel from './ChatAuxiliaryPanel.vue'
import ChatComposer from './ChatComposer.vue'
import ChatComposerSelector from './ChatComposerSelector.vue'
import ChatMarkdownRenderer from './ChatMarkdownRenderer.vue'
import ChatMessageItem from './ChatMessageItem.vue'
import ChatMessagePanel from './ChatMessagePanel.vue'
import ChatTimeline from './ChatTimeline.vue'
import ChatWorkspace from './ChatWorkspace.vue'
import { defaultMessages } from './messages'
import {
  applyChatStreamEvent, createChatThemeStyle, createPendingChatTurn,
  messagesToTimelineEntries, questionItemsFromTimeline,
} from './workspace'
import type { ChatMessage, ChatTimelineEntry, ChatWorkspaceActionEvent, ChatWorkspaceAdapter } from './types'

function message(id: string, role: ChatMessage['role'], text: string): ChatMessage {
  return { protocolVersion: 1, id, role, createdAt: 1, status: 'complete', parts: [{ type: 'text', text }] }
}

async function render(component: Parameters<typeof h>[0], props: Record<string, unknown>) {
  return renderToString(createSSRApp({ render: () => h(component, props) }))
}

describe('ChatWorkspace stage 1', () => {
  it('converts legacy messages and derives stable question navigation', () => {
    const entries = messagesToTimelineEntries([message('u1', 'user', 'first question'), message('a1', 'assistant', 'answer')])
    expect(entries.map(entry => entry.id)).toEqual(['message:u1', 'message:a1'])
    expect(questionItemsFromTimeline(entries)).toMatchObject([{ entryId: 'message:u1', label: 'first question', index: 0 }])
  })

  it('applies semantic stream events without mutating the prior timeline', () => {
    const turn = createPendingChatTurn({ text: 'hello' }, {
      turnId: 'turn-1', userMessageId: 'u1', assistantMessageId: 'a1', createdAt: 1,
    })
    const initial: ChatTimelineEntry[] = [turn]
    const streaming = applyChatStreamEvent(initial, { type: 'text-delta', delta: 'partial' }, { entryId: 'turn-1', messageId: 'a1' })
    const complete = applyChatStreamEvent(streaming, { type: 'message-complete', messageId: 'a1' })
    expect(initial[0]).toBe(turn)
    expect(turn.alternatives[0].parts).toEqual([])
    expect(streaming[0]).not.toBe(turn)
    expect((complete[0] as typeof turn).alternatives[0]).toMatchObject({ status: 'complete', parts: [{ type: 'text', text: 'partial' }] })
  })

  it('renders messages, model variants, and embedded plugins in one timeline', async () => {
    const Embedded = defineComponent({ props: ['data'], setup: props => () => h('div', { 'data-plan': '' }, String(props.data.title)) })
    const PartRenderer = defineComponent({ props: ['data'], setup: props => () => h('div', { 'data-part': '' }, String(props.data.value)) })
    const user = message('u1', 'user', 'compare')
    const entries: ChatTimelineEntry[] = [
      { kind: 'message', id: 'legacy', message: message('intro', 'system', 'intro') },
      { kind: 'turn', id: 'turn-1', user, alternatives: [
        { ...message('a1', 'assistant', 'one'), model: { id: 'm1', name: 'Model One' } },
        { ...message('a2', 'assistant', 'two'), model: { id: 'm2', name: 'Model Two' } },
      ], activeId: 'a1', comparisonMode: 'tabs' },
      { kind: 'embedded', id: 'plan-1', renderer: 'fastplan', data: { title: 'plan' } },
      { kind: 'message', id: 'custom-entry', message: {
        id: 'custom-message', role: 'assistant', createdAt: 2,
        parts: [{ type: 'custom', kind: 'metric', data: { value: 42 } }],
      } },
    ]
    const html = await render(ChatTimeline, { entries, plugins: {
      timelineRenderers: { fastplan: Embedded }, partRenderers: { metric: PartRenderer },
    } })
    expect(html).toContain('data-message-id="intro"')
    expect(html).toContain('data-turn-id="turn-1"')
    expect(html).toContain('Model One')
    expect(html).toContain('data-plan')
    expect(html).toContain('>plan</div>')
    expect(html).toContain('data-part')
    expect(html).toContain('>42</div>')
    expect(html).toContain('data-entry-id="turn-1"')
    expect(html.indexOf('data-message-id="u1"')).toBeLessThan(html.indexOf('jcr-variants__toolbar'))
    expect(html.indexOf('jcr-variants__toolbar')).toBeLessThan(html.indexOf('data-message-id="a1"'))
  })

  it('navigates turn questions to their timeline entry and marks the selected item active', async () => {
    const scrollIntoView = vi.fn()
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: scrollIntoView,
    })
    const wrapper = mount(ChatWorkspace, { props: {
      showQuestionNavigator: true,
      timeline: [{
        kind: 'turn', id: 'turn-1', user: message('u1', 'user', 'question'),
        alternatives: [message('a1', 'assistant', 'answer')], activeId: 'a1', comparisonMode: 'single',
      }, {
        kind: 'turn', id: 'turn-2', user: message('u2', 'user', 'second question'),
        alternatives: [message('a2', 'assistant', 'second answer')], activeId: 'a2', comparisonMode: 'single',
      }],
    } })
    await wrapper.get('.jcr-question-nav__rail-item').trigger('click')
    await flushPromises()
    expect(scrollIntoView).toHaveBeenCalled()
    expect(wrapper.get('.jcr-question-nav__rail-item').classes()).toContain('is-active')
    expect(wrapper.emitted('workspace-action')?.at(-1)?.[0]).toMatchObject({
      action: 'navigate', entryId: 'turn-1', scope: 'timeline',
    })
  })

  it('merges embedded question contributions and navigates to an inner message', async () => {
    const scrollIntoView = vi.fn()
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: scrollIntoView,
    })
    const Embedded = defineComponent({
      emits: ['action'],
      setup(_, { emit }) {
        onMounted(() => emit('action', { type: 'navigation-change', items: [
          { id: 'u1', label: 'first plan', targetMessageId: 'u1' },
          { id: 'u2', label: 'second plan', targetMessageId: 'u2' },
        ] }))
        return () => h('div', [
          h('article', { 'data-message-id': 'u1' }, 'first plan'),
          h('article', { 'data-message-id': 'u2' }, 'second plan'),
        ])
      },
    })
    const wrapper = mount(ChatWorkspace, { props: {
      showQuestionNavigator: true,
      timeline: [{ kind: 'embedded', id: 'plan-1', renderer: 'fastplan', data: {} }],
      plugins: { timelineRenderers: { fastplan: Embedded } },
    } })
    await flushPromises()

    const items = wrapper.findAll('.jcr-question-nav__rail-item')
    expect(items.map(item => item.text())).toEqual(['1first plan', '2second plan'])
    const target = wrapper.get('[data-message-id="u2"]').element
    await items[1].trigger('click')
    await flushPromises()

    expect(scrollIntoView).toHaveBeenCalled()
    expect(scrollIntoView.mock.contexts.at(-1)).toBe(target)
    expect(items[1].classes()).toContain('is-active')
    expect(wrapper.emitted('workspace-action')?.at(-1)?.[0]).toMatchObject({
      action: 'navigate', entryId: 'plan-1', messageId: 'u2', scope: 'timeline',
    })
  })

  it('opens a localized question directory beside the leading icon and navigates from it', async () => {
    const scrollIntoView = vi.fn()
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: scrollIntoView,
    })
    const wrapper = mount(ChatWorkspace, { props: {
      showQuestionNavigator: true,
      labels: {
        questionDirectory: '问题目录',
        openQuestionDirectory: '打开问题目录',
        closeQuestionDirectory: '关闭问题目录',
      },
      timeline: [{
        kind: 'turn', id: 'turn-1', user: message('u1', 'user', 'first question'),
        alternatives: [message('a1', 'assistant', 'answer')], activeId: 'a1', comparisonMode: 'single',
      }, {
        kind: 'turn', id: 'turn-2', user: message('u2', 'user', 'second question'),
        alternatives: [message('a2', 'assistant', 'second answer')], activeId: 'a2', comparisonMode: 'single',
      }],
    } })

    const nav = wrapper.get('.jcr-question-nav')
    const directory = wrapper.get('.jcr-question-nav__directory')
    expect(nav.element.firstElementChild).toBe(directory.element)
    expect(directory.attributes('aria-label')).toBe('打开问题目录')
    await directory.trigger('click')

    const panel = wrapper.get('.jcr-question-nav__panel')
    expect(panel.attributes('aria-label')).toBe('问题目录')
    expect(directory.attributes('aria-label')).toBe('关闭问题目录')
    expect(panel.findAll('.jcr-question-nav__panel-item').map(item => item.text())).toEqual([
      '1first question', '2second question',
    ])

    await panel.findAll('.jcr-question-nav__panel-item')[1].trigger('click')
    await flushPromises()
    expect(wrapper.find('.jcr-question-nav__panel').exists()).toBe(false)
    expect(scrollIntoView).toHaveBeenCalled()
    expect(wrapper.emitted('workspace-action')?.at(-1)?.[0]).toMatchObject({
      action: 'navigate', entryId: 'turn-2', scope: 'timeline',
    })

    await directory.trigger('click')
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await flushPromises()
    expect(wrapper.find('.jcr-question-nav__panel').exists()).toBe(false)

    await directory.trigger('click')
    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }))
    await flushPromises()
    expect(wrapper.find('.jcr-question-nav__panel').exists()).toBe(false)
    wrapper.unmount()
  })

  it('hides question navigation until more than one user question exists', () => {
    const wrapper = mount(ChatWorkspace, { props: {
      showQuestionNavigator: true,
      timeline: [{
        kind: 'turn', id: 'turn-1', user: message('u1', 'user', 'question'),
        alternatives: [message('a1', 'assistant', 'answer')],
      }],
    } })
    expect(wrapper.find('.jcr-question-nav').exists()).toBe(false)
  })

  it('shows the host-customizable scroll-to-bottom button only while away from the bottom', async () => {
    const scrollTo = vi.fn()
    const wrapper = mount(ChatWorkspace, {
      props: {
        timeline: [{ kind: 'message', id: 'message:a1', message: message('a1', 'assistant', 'answer') }],
        showScrollToBottom: true,
        labels: { scrollToBottom: '滚动至底部' },
      },
      slots: {
        'scroll-to-bottom-icon': '<span data-host-bottom-icon></span>',
      },
    })
    const timeline = wrapper.get('.jcr-timeline')
    Object.defineProperties(timeline.element, {
      scrollHeight: { configurable: true, value: 600 },
      clientHeight: { configurable: true, value: 200 },
      scrollTop: { configurable: true, writable: true, value: 100 },
      scrollTo: { configurable: true, value: scrollTo },
    })

    await timeline.trigger('scroll')
    const button = wrapper.get('.jcr-scroll-to-bottom')
    expect(button.attributes('title')).toBe('滚动至底部')
    expect(button.find('[data-host-bottom-icon]').exists()).toBe(true)

    await button.trigger('click')
    expect(scrollTo).toHaveBeenCalledWith({ top: 600, behavior: 'smooth' })
    expect(wrapper.find('.jcr-scroll-to-bottom').exists()).toBe(false)
  })

  it('drives ordinary chat through an optional adapter and emits normalized timeline updates', async () => {
    const adapter: ChatWorkspaceAdapter = {
      async *send(input) {
        yield { type: 'text-delta', delta: `echo:${input.text}` }
        yield { type: 'message-complete' }
      },
    }
    const wrapper = mount(ChatWorkspace, { props: { adapter, modelValue: 'hello', showContext: false } })
    await (wrapper.vm as unknown as { send: (value: unknown) => Promise<void> }).send({
      text: 'hello', attachments: [], nativeEvent: new Event('submit'),
    })
    const updates = wrapper.emitted('update:timeline') || []
    const finalTimeline = updates.at(-1)?.[0] as ChatTimelineEntry[]
    expect(finalTimeline).toHaveLength(1)
    expect(finalTimeline[0].kind).toBe('turn')
    expect((finalTimeline[0] as Extract<ChatTimelineEntry, { kind: 'turn' }>).alternatives[0]).toMatchObject({
      status: 'complete', parts: [{ type: 'text', text: 'echo:hello' }],
    })
    expect(wrapper.emitted('workspace-action')?.[0]?.[0]).toMatchObject({ action: 'submit', scope: 'composer' })
  })

  it('keeps an embedded planning entry visible while appending an ordinary answer', async () => {
    const planningEntry: ChatTimelineEntry = {
      kind: 'embedded', id: 'plan-1', renderer: 'fastplan', status: 'complete', data: {},
      metadata: { navigationLabel: 'plan question' },
    }
    const adapter: ChatWorkspaceAdapter = {
      createTimelineEntry: () => undefined,
      async *send() {
        yield { type: 'text-delta', delta: 'ordinary answer' }
        yield { type: 'message-complete' }
      },
    }
    const wrapper = mount(ChatWorkspace, { props: {
      adapter, timeline: [planningEntry], modelValue: 'ordinary question',
      showContext: false, showQuestionNavigator: true,
    } })
    await (wrapper.vm as unknown as { send: (value: unknown) => Promise<void> }).send({
      text: 'ordinary question', attachments: [], nativeEvent: new Event('submit'),
    })
    await flushPromises()

    const finalTimeline = wrapper.emitted('update:timeline')?.at(-1)?.[0] as ChatTimelineEntry[]
    expect(finalTimeline.map(entry => entry.kind)).toEqual(['embedded', 'turn'])
    expect(wrapper.text()).toContain('ordinary answer')
    expect(wrapper.findAll('.jcr-question-nav__rail-item')).toHaveLength(2)
  })

  it('stays display/event-only when no adapter is supplied', async () => {
    const wrapper = mount(ChatWorkspace, { props: { modelValue: 'hello', showContext: false } })
    await (wrapper.vm as unknown as { send: (value: unknown) => Promise<void> }).send({
      text: 'hello', attachments: [], nativeEvent: new Event('submit'),
    })
    expect(wrapper.emitted('workspace-action')?.[0]?.[0]).toMatchObject({ action: 'submit' })
    expect(wrapper.emitted('update:timeline')).toBeUndefined()
  })

  it('routes an opt-in submission to an embedded timeline entry without sending ordinary chat', async () => {
    let ordinarySendCount = 0
    const adapter: ChatWorkspaceAdapter = {
      createTimelineEntry: input => ({
        kind: 'embedded', id: 'fastplan-new', renderer: 'fastplan', status: 'active',
        data: { prompt: input.text },
      }),
      async *send() { ordinarySendCount += 1 },
    }
    const wrapper = mount(ChatWorkspace, { props: { adapter, modelValue: 'plan it', showContext: false } })
    await (wrapper.vm as unknown as { send: (value: unknown) => Promise<void> }).send({
      text: 'plan it', attachments: [], nativeEvent: new Event('submit'),
    })
    const finalTimeline = wrapper.emitted('update:timeline')?.at(-1)?.[0] as ChatTimelineEntry[]
    expect(finalTimeline).toEqual([expect.objectContaining({
      kind: 'embedded', id: 'fastplan-new', renderer: 'fastplan', data: { prompt: 'plan it' },
    })])
    expect(ordinarySendCount).toBe(0)
    expect(wrapper.emitted('update:modelValue')?.at(-1)?.[0]).toBe('')
  })

  it('can replace an active embedded entry during a routed submission', async () => {
    const active = {
      kind: 'embedded' as const, id: 'fastplan-active', renderer: 'fastplan', status: 'active' as const,
      data: { sessionId: 'session-1', command: { id: 'old' } },
    }
    const adapter: ChatWorkspaceAdapter = {
      createTimelineEntry: input => ({
        replaceEntryId: active.id,
        entry: { ...active, data: { ...active.data, command: { id: 'new', prompt: input.text } } },
      }),
      async *send() {},
    }
    const wrapper = mount(ChatWorkspace, { props: { adapter, timeline: [active], showContext: false } })
    await (wrapper.vm as unknown as { send: (value: unknown) => Promise<void> }).send({
      text: 'follow up', attachments: [], nativeEvent: new Event('submit'),
    })
    const finalTimeline = wrapper.emitted('update:timeline')?.at(-1)?.[0] as ChatTimelineEntry[]
    expect(finalTimeline).toHaveLength(1)
    expect((finalTimeline[0] as typeof active).data.command).toMatchObject({ id: 'new', prompt: 'follow up' })
  })

  it('emits standardized action results returned by a host adapter', async () => {
    const adapter: ChatWorkspaceAdapter = {
      async *send() {},
      executeAction: async event => ({ status: 'success', data: { action: event.action } }),
    }
    const assistant = message('a1', 'assistant', 'answer')
    const wrapper = mount(ChatWorkspace, { props: {
      adapter,
      timeline: [{
        kind: 'turn', id: 'turn-1', user: message('u1', 'user', 'question'),
        alternatives: [{ ...assistant, turnId: 'turn-1' }], activeId: 'a1', comparisonMode: 'single',
      }],
      actions: { like: true },
      resolveActions: () => ['like'],
    } })
    await wrapper.get('[aria-label="赞"]').trigger('click')
    await flushPromises()
    expect(wrapper.emitted('action-result')?.[0]?.[0]).toMatchObject({
      event: { action: 'like', messageId: 'a1' },
      result: { status: 'success', data: { action: 'like' } },
    })
  })

  it('routes parameter-menu selections through the workspace adapter', async () => {
    const executed: ChatWorkspaceActionEvent[] = []
    const adapter: ChatWorkspaceAdapter = {
      async *send() {},
      executeAction: async event => {
        executed.push(event)
        return { status: 'success' }
      },
    }
    const assistant = message('a1', 'assistant', 'answer')
    const wrapper = mount(ChatWorkspace, { props: {
      adapter,
      timeline: [{
        kind: 'turn', id: 'turn-1', user: message('u1', 'user', 'question'),
        alternatives: [{ ...assistant, turnId: 'turn-1' }], activeId: 'a1', comparisonMode: 'single',
      }],
      actions: {
        translate: {
          label: 'Translate',
          menuItems: [{ id: 'en', label: 'English', payload: { id: 'en', label: 'English' } }],
        },
      },
      resolveActions: message => message.role === 'assistant' ? ['translate'] : [],
    } })
    await wrapper.get('summary[aria-label="翻译"]').trigger('click')
    await wrapper.get('[role="menuitem"]').trigger('click')
    await flushPromises()
    expect(executed).toHaveLength(1)
    expect(executed[0]).toMatchObject({
      action: 'translate',
      messageId: 'a1',
      payload: {
        payload: { id: 'en', label: 'English' },
        menuItemId: 'en',
      },
    })
  })

  it('renders auxiliary fallbacks and attachment progress without global UI dependencies', async () => {
    const auxiliary = await render(ChatAuxiliaryPanel, {
      items: [{ id: 'trace', renderer: 'missing', title: 'Trace', data: {} }],
    })
    const composer = await render(ChatComposer, {
      selectors: [{ id: 'model', label: 'Model', value: 'm1', options: [{ value: 'm1', label: 'One' }] }],
      attachments: [{ id: 'f1', kind: 'file', name: 'report.pdf', status: 'uploading', progress: 42 }],
    })
    expect(auxiliary).toContain('渲染器不可用')
    expect(composer).toContain('role="progressbar"')
    expect(composer).toContain('width:42%')
    expect(composer).toContain('data-extension="PDF"')
    expect(composer.indexOf('jcr-composer__selectors')).toBeLessThan(composer.indexOf('jcr-composer__attachments'))
    expect(composer.indexOf('jcr-composer__attachments')).toBeLessThan(composer.indexOf('<textarea'))
  })

  it('keeps four semantic composer regions in order and exposes one shared slot scope', () => {
    let inputScope: Record<string, any> | undefined
    let toolbarScope: Record<string, any> | undefined
    const wrapper = mount(ChatWorkspace, {
      props: {
        modelValue: 'draft',
        composerSelectors: [{ id: 'model', label: 'Model', value: 'm1', options: [{ value: 'm1', label: 'One' }] }],
        composerAttachments: [{ id: 'file', kind: 'file', name: 'report.pdf' }],
        composerActions: [{ id: 'thinking', label: 'Thinking' }],
      },
      slots: {
        'composer-input': (scope: Record<string, any>) => {
          inputScope = scope
          return h('div', { 'data-custom-input': '' }, scope.value)
        },
        'composer-toolbar-end': (scope: Record<string, any>) => {
          toolbarScope = scope
          return h('i', { 'data-toolbar-scope': '' })
        },
      },
    })
    const regions = wrapper.get('.jcr-composer__box').findAll('[data-composer-region]')
      .map(region => region.attributes('data-composer-region'))
    expect(regions).toEqual(['selectors', 'attachments', 'input', 'toolbar'])
    expect(wrapper.get('[data-custom-input]').text()).toBe('draft')
    for (const scope of [inputScope, toolbarScope]) {
      expect(scope).toMatchObject({ value: 'draft', disabled: false, readonly: false, loading: false, canSubmit: true })
      expect(scope?.selectors).toHaveLength(1)
      expect(scope?.attachments).toHaveLength(1)
      expect(scope?.actions).toHaveLength(1)
      expect(scope?.update).toEqual(expect.any(Function))
      expect(scope?.submit).toEqual(expect.any(Function))
      expect(scope?.abort).toEqual(expect.any(Function))
      expect(scope?.triggerAction).toEqual(expect.any(Function))
    }
  })

  it('emits a document reference action from a user attachment', async () => {
    const value: ChatMessage = {
      ...message('u-file', 'user', 'question'),
      parts: [{ type: 'attachment', id: 'file-1', name: 'report.pdf', metadata: { source: 'host' } }],
    }
    const wrapper = mount(ChatMessageItem, { props: { message: value, messages: defaultMessages } })
    await wrapper.get('.jcr-attachment__reference').trigger('click')
    expect(wrapper.emitted('message-action')?.[0]?.[0]).toMatchObject({
      action: 'custom',
      messageId: 'u-file',
      payload: { type: 'attachment-reference', attachment: { id: 'file-1', name: 'report.pdf' } },
    })
  })

  it('renders image attachments as thumbnail-only cards', () => {
    const wrapper = mount(ChatComposer, { props: {
      attachments: [{ id: 'image-1', kind: 'image', name: 'reference.png', size: 2048, previewUrl: 'data:image/png;base64,AA==', status: 'complete' }],
    } })
    const card = wrapper.get('.jcr-composer-attachment--image')
    expect(card.get('img').attributes('src')).toBe('data:image/png;base64,AA==')
    expect(card.find('.jcr-composer-attachment__info').exists()).toBe(false)
    expect(card.get('.jcr-composer-attachment__status--complete').text()).toBe('✓')
  })

  it('uses the packaged MdPreview highlighter while stripping styles from enabled raw HTML', async () => {
    const highlighted = mount(ChatMarkdownRenderer, {
      props: { content: '```javascript\nconst answer = 42\n```', theme: 'light' },
    })
    await vi.waitFor(() => expect(highlighted.find('.hljs-keyword').exists()).toBe(true))
    expect(highlighted.find('.md-editor-preview').exists()).toBe(true)
    expect(highlighted.find('[rn-wrapper]').exists()).toBe(true)
    expect(highlighted.find('.md-editor-collapse-tips svg').exists()).toBe(true)

    await highlighted.setProps({ theme: 'dark' })
    await vi.waitFor(() => expect(highlighted.get('.md-editor').classes()).toContain('md-editor-dark'))

    const rawHtml = mount(ChatMarkdownRenderer, {
      props: { content: '<span style="color:red">external</span>', allowHtml: true },
    })
    expect(rawHtml.get('.jcr-markdown span').attributes('style')).toBeUndefined()
  })

  it('renders image parts through MdPreview so base64 images get built-in zoom', async () => {
    const wrapper = mount(ChatMessagePanel, {
      props: {
        messages: [{
          id: 'user-image',
          role: 'user',
          createdAt: 1,
          status: 'complete',
          parts: [{
            type: 'image',
            src: 'data:image/png;base64,iVBORw0KGgo=',
            alt: 'preview',
          }],
        }],
      },
    })
    await flushPromises()
    const image = wrapper.get('.jcr-markdown--image .md-editor-preview img')
    expect(image.classes()).toContain('md-zoom')
    expect(image.attributes('src')).toContain('data:image/png;base64,')
  })

  it('configures code folding and line numbers through the renderer and workspace', async () => {
    const direct = mount(ChatMarkdownRenderer, { props: { content: '```text\nvalue\n```' } })
    expect(direct.props('autoFoldThreshold')).toBe(10000)
    expect(direct.props('showCodeRowNumber')).toBe(true)

    const workspace = mount(ChatWorkspace, {
      props: {
        showComposer: false,
        markdownAutoFoldThreshold: 321,
        markdownShowCodeRowNumber: false,
        timeline: [{
          kind: 'turn',
          id: 'fold-turn',
          user: message('fold-user', 'user', 'question'),
          alternatives: [message('fold-answer', 'assistant', '```text\nanswer\n```')],
          activeId: 'fold-answer',
        }],
      },
    })
    await flushPromises()
    expect(workspace.getComponent(ChatMarkdownRenderer).props('autoFoldThreshold')).toBe(321)
    expect(workspace.getComponent(ChatMarkdownRenderer).props('showCodeRowNumber')).toBe(false)
  })

  it('forwards the composer auto-resize limit and yields height control to the host', async () => {
    const wrapper = mount(ChatWorkspace, {
      props: { modelValue: '', composerMaxHeight: 153, composerAutoResize: true },
    })
    const textarea = wrapper.get('textarea').element as HTMLTextAreaElement
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 240 })

    await wrapper.setProps({ modelValue: 'line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7' })
    await flushPromises()
    expect(textarea.style.height).toBe('153px')
    expect(textarea.style.overflowY).toBe('auto')

    await wrapper.setProps({ composerAutoResize: false })
    await flushPromises()
    expect(textarea.style.height).toBe('')
    expect(textarea.style.overflowY).toBe('auto')
  })

  it('keeps composer selectors controlled and emits normalized single and multiple values', async () => {
    const single = mount(ChatComposerSelector, { props: { selector: {
      id: 'model', label: 'Model', value: 'm1', searchable: true,
      options: [{ value: 'm1', label: 'One' }, { value: 'm2', label: 'Two', group: 'Cloud' }],
    } } })
    single.get('details').element.setAttribute('open', '')
    await single.findAll('.jcr-composer-selector__options button')[1].trigger('click')
    expect(single.emitted('change')?.[0]?.[0]).toMatchObject({ value: 'm2', selected: [{ value: 'm2', label: 'Two', group: 'Cloud' }] })

    const multiple = mount(ChatComposerSelector, { props: { selector: {
      id: 'knowledge', label: 'Knowledge', value: ['k1'], multiple: true,
      options: [{ value: 'k1', label: 'One' }, { value: 'k2', label: 'Two' }],
    } } })
    await multiple.findAll('.jcr-composer-selector__options button')[1].trigger('click')
    expect(multiple.emitted('change')?.[0]?.[0]).toMatchObject({ value: ['k1', 'k2'] })
  })

  it('uses an explicit file action without changing ordinary button action semantics', async () => {
    const wrapper = mount(ChatComposer, { props: { actions: [
      { id: 'upload', label: 'Upload', kind: 'file', accept: '.pdf', multiple: true },
      { id: 'think', label: 'Think' },
    ] } })
    await wrapper.findAll('.jcr-composer__actions button')[1].trigger('click')
    expect(wrapper.emitted('composer-action')?.[0]?.[0]).toMatchObject({ action: 'think' })

    const input = wrapper.get('input[type="file"]')
    const file = new File(['data'], 'report.pdf', { type: 'application/pdf' })
    Object.defineProperty(input.element, 'files', { configurable: true, value: [file] })
    await input.trigger('change')
    expect(wrapper.emitted('files-selected')?.[0]?.[0]).toMatchObject({ action: 'upload', files: [file] })
  })

  it('keeps file selection working through a host-rendered workspace action slot', async () => {
    const wrapper = mount(ChatWorkspace, {
      props: { composerActions: [{ id: 'image', label: 'Image', kind: 'file', accept: '.png', multiple: true }] },
      slots: {
        'composer-action': ({ action, trigger }: { action: { id: string }; trigger: (action: unknown, event: MouseEvent) => void }) =>
          h('button', { type: 'button', 'data-host-file-action': action.id, onClick: (event: MouseEvent) => trigger(action, event) }, action.id),
      },
    })
    await wrapper.get('[data-host-file-action="image"]').trigger('click')

    const input = wrapper.get('input[type="file"]')
    const file = new File(['image'], 'sample.png', { type: 'image/png' })
    Object.defineProperty(input.element, 'files', { configurable: true, value: [file] })
    await input.trigger('change')

    expect(wrapper.emitted('composer-files')?.[0]?.[0]).toMatchObject({ action: 'image', files: [file] })
    expect(wrapper.emitted('workspace-action')?.at(-1)?.[0]).toMatchObject({
      action: 'composer-action', scope: 'composer', actionId: 'image',
    })
  })

  it('forwards a host-provided workspace empty state', async () => {
    const wrapper = mount(ChatWorkspace, { slots: { empty: '<div data-host-empty>Welcome</div>' } })
    expect(wrapper.find('[data-host-empty]').text()).toBe('Welcome')
  })

  it('forwards an opt-in host selector renderer without changing the default contract', async () => {
    const selectors = [{ id: 'model', label: 'Model', value: 'm1', options: [{ value: 'm1', label: 'One' }] }]
    const wrapper = mount(ChatWorkspace, {
      props: { composerSelectors: selectors },
      slots: {
        'composer-selectors': ({ selectors: value }: { selectors: typeof selectors }) =>
          h('div', { 'data-host-selectors': value.length }, value[0].label),
      },
    })
    expect(wrapper.get('[data-host-selectors="1"]').text()).toBe('Model')
    expect(wrapper.find('.jcr-composer-selector').exists()).toBe(false)
  })

  it('forwards composer inside-top content into the input box', () => {
    const wrapper = mount(ChatWorkspace, {
      slots: { 'composer-inside-top': '<div data-inside-top>Resize</div>' },
    })
    const inputBox = wrapper.get('.jcr-composer__box')
    expect(inputBox.get('[data-inside-top]').text()).toBe('Resize')
    expect(inputBox.element.firstElementChild?.hasAttribute('data-inside-top')).toBe(true)
  })

  it('renders a host selector slot even when the host does not use public selector data', async () => {
    const wrapper = mount(ChatWorkspace, {
      slots: { 'composer-selectors': () => h('div', { 'data-host-selectors-only': '' }, 'Host model and knowledge') },
    })
    expect(wrapper.get('[data-host-selectors-only]').text()).toBe('Host model and knowledge')
    expect(wrapper.find('.jcr-composer__selectors').exists()).toBe(true)
  })

  it('forwards optional host icon renderers while preserving workspace actions', async () => {
    const timeline = [{
      kind: 'turn' as const, id: 'turn-icons', user: message('u-icons', 'user', 'question'),
      alternatives: [message('a-icons', 'assistant', 'answer')],
    }]
    const wrapper = mount(ChatWorkspace, {
      props: {
        timeline,
        composerActions: [{ id: 'upload-image', label: 'Upload image' }],
        resolveActions: (value: ChatMessage) => value.role === 'assistant' ? ['copy'] : [],
      },
      slots: {
        'composer-action-icon': ({ action }: { action: { id: string } }) => h('i', { 'data-composer-icon': action.id }),
        'composer-submit-icon': () => h('i', { 'data-submit-icon': '' }),
        'message-action-icon': ({ iconName }: { iconName: string }) => h('i', { 'data-message-icon': iconName }),
      },
    })
    expect(wrapper.find('[data-composer-icon="upload-image"]').exists()).toBe(true)
    expect(wrapper.find('[data-submit-icon]').exists()).toBe(true)
    expect(wrapper.find('[data-message-icon="copy"]').exists()).toBe(true)

    const loading = mount(ChatWorkspace, {
      props: { composerLoading: true },
      slots: { 'composer-abort-icon': () => h('i', { 'data-abort-icon': '' }) },
    })
    expect(loading.find('[data-abort-icon]').exists()).toBe(true)
  })

  it('maps theme tokens only to jcr-scoped CSS variables', () => {
    expect(createChatThemeStyle({ primary: '#123456', auxiliaryWidth: '400px' })).toEqual({
      '--jcr-primary': '#123456', '--jcr-auxiliary-width': '400px',
    })
  })

  it('propagates workspace theme tokens to nested conversation, timeline, and navigator roots', () => {
    const wrapper = mount(ChatWorkspace, { props: {
      showQuestionNavigator: true,
      themeTokens: { background: '#f8fafc' },
      timeline: [{
        kind: 'turn', id: 'turn-1', user: message('u1', 'user', 'question'),
        alternatives: [message('a1', 'assistant', 'answer')],
      }, {
        kind: 'turn', id: 'turn-2', user: message('u2', 'user', 'second question'),
        alternatives: [message('a2', 'assistant', 'second answer')],
      }],
    } })
    expect(wrapper.get('.jcr-conversation').attributes('style')).toContain('--jcr-bg: #f8fafc')
    expect(wrapper.get('.jcr-timeline').attributes('style')).toContain('--jcr-bg: #f8fafc')
    expect(wrapper.get('.jcr-question-nav').attributes('style')).toContain('--jcr-bg: #f8fafc')
  })
})

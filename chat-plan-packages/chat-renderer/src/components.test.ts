import { readFileSync } from 'node:fs'
import { createSSRApp, h } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { describe, expect, it } from 'vitest'
import ChatMessageItem from './ChatMessageItem.vue'
import ChatMessagePanel from './ChatMessagePanel.vue'
import ChatConversation from './ChatConversation.vue'
import ChatActionIcon from './ChatActionIcon.vue'
import ChatModelIcon from './ChatModelIcon.vue'
import chatMarkdownRendererSource from './ChatMarkdownRenderer.vue?raw'
import { defaultMessages } from './messages'
import type { ChatMessage, ResolvedMessageAction } from './types'

const chatStyleSource = readFileSync(new URL('./style.css', import.meta.url), 'utf8')
const workspaceSource = readFileSync(new URL('./ChatWorkspace.vue', import.meta.url), 'utf8')
const timelineSource = readFileSync(new URL('./ChatTimeline.vue', import.meta.url), 'utf8')
const variantsSource = readFileSync(new URL('./ChatModelVariants.vue', import.meta.url), 'utf8')

function message(id: string, role: ChatMessage['role'], model?: string): ChatMessage {
  return {
    protocolVersion: 1, id, role, createdAt: '2026-08-24T08:00:00.000Z',
    model: model ? { id: model, name: model } : undefined,
    parts: [{ type: 'text', text: id, format: 'plain' }],
  }
}

function action(name: ResolvedMessageAction['name'], order: number): ResolvedMessageAction {
  return { name, actionId: `${name}-${order}`, label: name, order, visible: true, disabled: false }
}

async function render(component: Parameters<typeof h>[0], props: Record<string, unknown>, slots?: Record<string, unknown>) {
  return renderToString(createSSRApp({ render: () => h(component, props, slots) }))
}

describe('chat renderer component contracts', () => {
  it.each([true, false])('shows one spinner for nested loading conversations (outer loading: %s)', async outerLoading => {
    const html = await render(ChatConversation, { contentLoading: outerLoading, showComposer: false }, {
      messages: () => h(ChatConversation, { initializing: true, showComposer: false }),
    })
    expect(html.match(/class="jcr-conversation__loading"/g)).toHaveLength(1)
    expect(html.match(/class="jcr-conversation"/g)).toHaveLength(2)
  })

  it('initializes packaged Markdown extensions through an explicit retained call', () => {
    expect(chatMarkdownRendererSource).toContain("import { ensureMarkdownExtensions } from './markdownExtensions'")
    expect(chatMarkdownRendererSource).toContain('ensureMarkdownExtensions()')
    expect(chatMarkdownRendererSource).not.toContain("import './markdownExtensions'")
  })

  it('uses the configured full-content and compact-composer widths by default', () => {
    expect(chatStyleSource).toContain('padding:var(--jcr-timeline-padding,')
    expect(chatStyleSource).toContain('.jcr-turn { display: grid; width:100%; min-width:0;')
    expect(chatStyleSource).toContain('.jcr-timeline__embedded { width:100%;')
    expect(chatStyleSource).toContain('--jcr-bubble-max-width:95%')
    expect(chatStyleSource).toContain('.jcr-message--user .jcr-message__main { width:auto; max-width:var(--jcr-bubble-max-width); }')
    expect(chatStyleSource).toContain('.jcr-message--user .jcr-message__main{max-width:90%}')
    expect(chatStyleSource).toContain('.jcr-context-bar[data-placement="composer"] { width:95%;')
    expect(chatStyleSource).toContain('.jcr-composer { position:relative; width:95%; margin:0 auto;')
    expect(chatStyleSource).not.toContain('calc((100% - 960px)/2)')
    expect(chatStyleSource).not.toContain('--jcr-bubble-max-width:78%')
    expect(chatStyleSource).not.toContain('width:min(100%, 960px)')
    expect(chatStyleSource).toContain('.md-editor-code { max-width:100%; overflow:visible;')
    expect(chatStyleSource).toContain('.md-editor-code-head { position:sticky; top:0;')
    expect(chatStyleSource).not.toContain('.md-editor-code { max-width:100%; overflow:auto;')
  })

  it('uses the legacy left-side hover question navigation by default', () => {
    expect(chatStyleSource).toContain('top:44%; left:8px;')
    expect(chatStyleSource).toContain('max-width:22px; min-height:22px;')
    expect(chatStyleSource).toContain('.jcr-question-nav__rail-item:hover,.jcr-question-nav__rail-item:focus-visible { max-width:220px;')
    expect(chatStyleSource).toContain('.jcr-question-nav__rail-item.is-active .jcr-question-nav__index { color:#fff; background:var(--jcr-primary); }')
    expect(chatStyleSource).toContain('.jcr-question-nav__label { max-width:0;')
    expect(chatStyleSource).toContain('.jcr-question-nav__panel { position:absolute; top:4px; left:30px;')
    expect(chatStyleSource).toContain('max-height:calc(100% - 8px); overflow-x:hidden; overflow-y:auto;')
    expect(chatStyleSource).toContain('grid-template-columns:max-content minmax(0,1fr);')
    expect(chatStyleSource).toContain('.jcr-question-nav__panel-index { min-width:7px;')
    expect(chatStyleSource).not.toContain('top:50%; right:12px;')
    expect(chatStyleSource).not.toContain('.jcr-question-nav { right:5px; }')
  })

  it('supports host scrolling and omits the built-in composer when embedded', async () => {
    const html = await render(ChatConversation, { messages: [], showComposer: false, scrollMode: 'host' })
    expect(html).toContain('data-scroll-mode="host"')
    expect(html).not.toContain('jcr-composer')
    expect(html).not.toContain('data-appearance')
    expect(html).not.toContain('jcr-conversation__auxiliary')
  })
  it('enables seamless host appearance and composer-independent auxiliary content only when requested', async () => {
    const html = await render(ChatConversation, {
      messages: [], showComposer: false, showComposerAuxiliary: true, appearance: 'seamless',
    }, {
      'composer-before': () => h('div', { 'data-inline-confirmation': '' }, 'confirm'),
    })
    expect(html).toContain('data-appearance="seamless"')
    expect(html).toContain('jcr-conversation__auxiliary')
    expect(html).toContain('data-inline-confirmation')
    expect(html).not.toContain('jcr-composer')
  })
  it('renders the migrated project model SVGs and keeps a neutral fallback', async () => {
    const html = await render('div', {}, {
      default: () => [
        h(ChatModelIcon, { name: 'gpt-5' }),
        h(ChatModelIcon, { name: 'Kimi K2' }),
        h(ChatModelIcon, { name: 'SiliconFlow' }),
        h(ChatModelIcon, { name: 'unknown-model' }),
      ],
    })
    expect(html).toContain('data-model-icon="OPENAI"')
    expect(html).toContain('data-model-icon="MOONSHOT"')
    expect(html.match(/jcr-model-icon--brand/g)?.length).toBe(3)
    expect(html).toContain('data-model-icon="CI"')
    expect(html).toContain('data:image/png;base64,')
    expect(html).toContain('data-model-icon="MODEL"')
    expect(html).toContain('jcr-model-icon--fallback')
  })

  it('provides the host action bar icons and auxiliary states without Lucide', async () => {
    const names = ['edit-as-doc', 'switch-model', 'trace-source', 'feedback', 'quote', 'chevron-down', 'loader'] as const
    const html = await render('div', {}, {
      default: () => names.map(name => h(ChatActionIcon, { name })),
    })
    for (const name of names) expect(html).toContain(`data-action-icon="${name}"`)
    expect(html).toContain('jcr-action__svg--spin')
    expect(html).toContain('M20 18v-2a4 4 0 0 0-4-4H4')
  })

  it('treats built-in and aliased icon overrides as SVG names while preserving text glyphs', async () => {
    const html = await render('div', {}, {
      default: () => [
        h(ChatActionIcon, { name: 'custom', customIcon: 'switch-model' }),
        h(ChatActionIcon, { name: 'custom', customIcon: 'trace' }),
        h(ChatActionIcon, { name: 'custom', customIcon: '✨' }),
      ],
    })
    expect(html).toContain('data-action-icon="switch-model"')
    expect(html).toContain('data-action-icon="trace-source"')
    expect(html).not.toContain('>switch-model<')
    expect(html).not.toContain('>trace<')
    expect(html).toContain('jcr-action__custom-icon')
    expect(html).toContain('✨')
  })

  it('server-renders the five-action fallback plus overflow access', async () => {
    const actions = ['copy', 'regenerate', 'quote', 'tts', 'translate', 'favorite', 'export', 'like']
      .map((name, index) => action(name as ResolvedMessageAction['name'], index))
    const html = await render(ChatMessageItem, { message: message('a1', 'assistant'), actions, messages: defaultMessages })
    expect(html.match(/class="jcr-action"/g)?.length).toBe(6)
    expect(html).toContain('class="jcr-more"')
    expect(html).toContain('aria-label="更多"')
  })

  it('uses host messages for direct and menu action tooltips and accessible labels', async () => {
    const actions = [
      action('copy', 0),
      { ...action('translate', 1), menuItems: [{ id: 'zh', label: '中文' }] },
    ]
    const html = await render(ChatMessageItem, {
      message: message('a1', 'assistant'),
      actions,
      messages: { ...defaultMessages, copy: '复制', translate: '翻译' },
    })
    expect(html).toContain('title="复制"')
    expect(html).toContain('aria-label="复制"')
    expect(html).toContain('title="翻译"')
    expect(html).toContain('aria-label="翻译"')
  })

  it('passes the message and a safe icon name when the overflow trigger has no action', async () => {
    const value = message('a1', 'assistant')
    const actions = ['copy', 'regenerate', 'quote', 'tts', 'translate', 'favorite', 'export', 'like']
      .map((name, index) => action(name as ResolvedMessageAction['name'], index))
    const html = await render(ChatMessageItem, { message: value, actions, messages: defaultMessages }, {
      'action-icon': ({ action: item, message: slotMessage, iconName }: {
        action?: ResolvedMessageAction; message: ChatMessage; iconName: string
      }) => h('i', {
        'data-slot-action': item?.name || 'overflow',
        'data-slot-message': slotMessage.id,
        'data-slot-icon': iconName,
      }),
    })
    expect(html).toContain('data-slot-action="overflow"')
    expect(html).toContain('data-slot-message="a1"')
    expect(html).toContain('data-slot-icon="custom"')
  })

  it('forwards host after-content slots through timeline and model variants', () => {
    expect(workspaceSource).toContain("$slots['message-after-content']")
    expect(timelineSource).toContain("$slots['after-content']")
    expect(variantsSource).toContain("$slots['after-content']")
  })

  it('renders host-provided translations and loading action indicators', async () => {
    const value = {
      ...message('a1', 'assistant'),
      translations: [{
        ...message('translation-1', 'assistant'),
        parts: [{ type: 'text' as const, text: 'translated', format: 'plain' as const }],
        metadata: { label: '翻译结果 · 英文' },
      }],
    }
    const html = await render(ChatMessageItem, {
      message: value,
      actions: [{ ...action('translate', 0), loading: true, disabled: true }],
      messages: defaultMessages,
    })
    expect(html).toContain('jcr-translations')
    expect(html).toContain('翻译结果 · 英文')
    expect(html).toContain('data-action-icon="loader"')
  })

  it('does not duplicate message streaming dots when reasoning already exposes progress', async () => {
    const value: ChatMessage = {
      ...message('a-stream', 'assistant'),
      status: 'streaming',
      parts: [{ type: 'reasoning', text: 'thinking', status: 'streaming' }],
    }
    const html = await render(ChatMessageItem, { message: value, actions: [], messages: defaultMessages }, {
      reasoning: () => h('span', { class: 'jcr-live-dot' }),
    })
    expect(html).toContain('jcr-live-dot')
    expect(html).not.toContain('jcr-streaming')
  })

  it('renders an explicit divider before a direct action', async () => {
    const actions = [action('copy', 0), { ...action('quote', 1), dividerBefore: true }]
    const html = await render(ChatMessageItem, { message: message('a1', 'assistant'), actions, messages: defaultMessages })
    expect(html.match(/class="jcr-action-divider"/g)?.length).toBe(1)
  })

  it('renders multiple custom action menus without replacing parameter menus', async () => {
    const actions: ResolvedMessageAction[] = [
      action('copy', 0),
      { ...action('translate', 1), customMenu: true },
      { ...action('export', 2), customMenu: true },
      { ...action('quote', 3), menuItems: [{ id: 'plain', label: 'Parameter menu' }] },
    ]
    const html = await render(ChatMessageItem, { message: message('a1', 'assistant'), actions, messages: defaultMessages }, {
      'action-menu': ({ action: value, placement }: { action: ResolvedMessageAction; placement: string }) =>
        h('span', { 'data-custom-menu': value.name, 'data-placement': placement }, `${value.name} menu`),
    })
    expect(html).toContain('data-custom-menu="translate"')
    expect(html).toContain('data-custom-menu="export"')
    expect(html.match(/data-placement="direct"/g)?.length).toBe(2)
    expect(html.match(/jcr-action-menu__popup--custom/g)?.length).toBe(2)
    expect(html).toContain('Parameter menu')
    expect(html).not.toContain('data-custom-menu="quote"')
  })

  it('passes overflow placement to custom action menus', async () => {
    const actions = ['copy', 'regenerate', 'quote', 'tts', 'favorite', 'like', 'export']
      .map((name, index) => ({
        ...action(name as ResolvedMessageAction['name'], index),
        customMenu: name === 'export',
      }))
    const html = await render(ChatMessageItem, { message: message('a1', 'assistant'), actions, messages: defaultMessages }, {
      'action-menu': ({ action: value, placement }: { action: ResolvedMessageAction; placement: string }) =>
        h('span', { 'data-custom-menu': value.name, 'data-placement': placement }, value.name),
    })
    expect(html).toContain('data-custom-menu="export"')
    expect(html).toContain('data-placement="overflow"')
  })

  it('keeps every user action directly visible without an overflow menu', async () => {
    const actions = ['copy', 'edit', 'resend', 'custom', 'quote', 'tts', 'favorite', 'export']
      .map((name, index) => action(name as ResolvedMessageAction['name'], index))
    const html = await render(ChatMessageItem, { message: message('u1', 'user'), actions, messages: defaultMessages })
    expect(html.match(/class="jcr-action"/g)?.length).toBe(actions.length)
    expect(html).not.toContain('class="jcr-more"')
    expect(html).not.toContain('aria-label="More"')
  })

  it('keeps resolved user actions direct through the message panel', async () => {
    const userActions = ['copy', 'edit', 'resend', 'custom', 'quote', 'tts', 'favorite', 'export']
    const html = await render(ChatMessagePanel, {
      messages: [message('u1', 'user')],
      actions: Object.fromEntries(userActions.map(name => [name, true])),
      resolveActions: () => userActions,
    })
    expect(html.match(/class="jcr-action"/g)?.length).toBe(userActions.length)
    expect(html).not.toContain('class="jcr-more"')
    expect(html).not.toContain('aria-label="More"')
  })

  it('renders one model header per turn and a neutral user icon', async () => {
    const html = await render(ChatMessagePanel, {
      messages: [message('u1', 'user'), message('a1', 'assistant', 'gpt-5'), message('a2', 'assistant', 'gpt-5')],
    })
    expect(html.match(/class="jcr-message__header"/g)?.length).toBe(1)
    expect(html.match(/class="jcr-user-icon"/g)?.length).toBe(1)
    expect(html).toContain('class="jcr-message__time"')
  })

  it('renders the turn model header before a leading tool group', async () => {
    const tool = (id: string): ChatMessage => ({
      protocolVersion: 1, id, role: 'tool', createdAt: 1, status: 'complete',
      model: { id: 'gpt-5', name: 'gpt-5' },
      parts: [{ type: 'tool-call', id, name: 'Read', status: 'complete' }],
    })
    const html = await render(ChatMessagePanel, {
      messages: [message('u1', 'user'), tool('t1'), tool('t2'), message('a1', 'assistant', 'gpt-5')],
    })
    const userIndex = html.indexOf('data-message-id="u1"')
    const headerIndex = html.indexOf('class="jcr-message__header"')
    const toolGroupIndex = html.indexOf('class="jcr-tool-group"')
    const answerIndex = html.indexOf('data-message-id="a1"')
    expect(headerIndex).toBeGreaterThan(userIndex)
    expect(toolGroupIndex).toBeGreaterThan(headerIndex)
    expect(answerIndex).toBeGreaterThan(toolGroupIndex)
    expect(html.match(/class="jcr-message__header"/g)?.length).toBe(1)
  })

  it('shows a fallback model header while a leading tool is the only model output', async () => {
    const tool: ChatMessage = {
      protocolVersion: 1, id: 't1', role: 'tool', createdAt: 1, status: 'streaming',
      model: { id: 'gpt-5', name: 'gpt-5' },
      parts: [{ type: 'tool-call', id: 't1', name: 'Read', status: 'running' }],
    }
    const html = await render(ChatMessagePanel, { messages: [message('u1', 'user'), tool] })
    expect(html.indexOf('class="jcr-message__header"')).toBeLessThan(html.indexOf('data-message-id="t1"'))
    expect(html).toContain('>gpt-5</span>')
  })

  it('keeps the public action bar on the final assistant answer before turn metadata', async () => {
    const html = await render(ChatMessagePanel, {
      messages: [
        message('u1', 'user'),
        message('a1', 'assistant', 'gpt-5'),
        message('tail', 'system'),
      ],
      actions: { quote: true },
    })
    const answerStart = html.indexOf('data-message-id="a1"')
    const tailStart = html.indexOf('data-message-id="tail"')
    const actionsStart = html.indexOf('class="jcr-actions"', answerStart)
    expect(answerStart).toBeGreaterThan(-1)
    expect(actionsStart).toBeGreaterThan(answerStart)
    expect(actionsStart).toBeLessThan(tailStart)
  })

  it('renders before-content between the model header and answer with turn context', async () => {
    const html = await render(ChatMessagePanel, {
      messages: [message('u1', 'user'), message('a1', 'assistant', 'gpt-5')],
    }, {
      'before-content': ({ message: value, turn, messageIndex, isFirstAssistantInTurn }: {
        message: ChatMessage
        turn: { id: string }
        messageIndex: number
        isFirstAssistantInTurn: boolean
      }) => value.role === 'assistant'
        ? h('span', { 'data-before-content': '', 'data-turn': turn.id }, `${value.id}:${messageIndex}:${isFirstAssistantInTurn}`)
        : null,
    })
    const headerIndex = html.indexOf('class="jcr-message__header"')
    const slotIndex = html.indexOf('data-before-content')
    const contentIndex = html.indexOf('class="jcr-message__content"', slotIndex)
    expect(headerIndex).toBeGreaterThan(-1)
    expect(slotIndex).toBeGreaterThan(headerIndex)
    expect(contentIndex).toBeGreaterThan(slotIndex)
    expect(html).toContain('a1:1:true</span>')
  })

  it('exposes after-content and message-after with message context', async () => {
    const html = await render(ChatMessagePanel, { messages: [message('u1', 'user')] }, {
      'after-content': ({ message: value }: { message: ChatMessage }) => h('span', { 'data-after-content': '' }, value.id),
      'message-after': ({ message: value, messageIndex }: { message: ChatMessage; messageIndex: number }) => h('span', { 'data-message-after': '' }, `${value.id}:${messageIndex}`),
    })
    expect(html).toContain('data-after-content>u1</span>')
    expect(html).toContain('data-message-after>u1:0</span>')
  })

  it('lets content replace all parts while preserving the public shell order', async () => {
    const html = await render(ChatMessagePanel, {
      messages: [message('a1', 'assistant', 'gpt-5')],
      actions: { quote: true },
    }, {
      'before-content': () => h('span', { 'data-order': 'before' }, 'before'),
      content: ({ parts }: { parts: ChatMessage['parts'] }) => h('span', { 'data-order': 'content' }, `custom:${parts.length}`),
      text: () => h('span', { 'data-should-not-render': '' }, 'part'),
      'after-content': () => h('span', { 'data-order': 'after' }, 'after'),
      'message-after': () => h('span', { 'data-order': 'message-after' }, 'message-after'),
    })
    const header = html.indexOf('class="jcr-message__header"')
    const before = html.indexOf('data-order="before"')
    const content = html.indexOf('data-order="content"')
    const after = html.indexOf('data-order="after"')
    const actions = html.indexOf('class="jcr-actions"')
    const messageAfter = html.indexOf('data-order="message-after"')
    expect([header, before, content, after, actions, messageAfter]).toEqual(
      [...[header, before, content, after, actions, messageAfter]].sort((a, b) => a - b),
    )
    expect(html).toContain('custom:1')
    expect(html).not.toContain('data-should-not-render')
    expect(html).not.toContain('>a1</div>')
  })

  it('replaces only matching text and reasoning parts when content is not overridden', async () => {
    const value = message('a1', 'assistant', 'gpt-5')
    value.parts = [
      { type: 'text', text: 'default text', format: 'markdown' },
      { type: 'reasoning', text: 'default reasoning' },
      { type: 'notice', text: 'default notice' },
    ]
    const html = await render(ChatMessagePanel, { messages: [value] }, {
      text: ({ partIndex }: { partIndex: number }) => h('span', { 'data-text-index': partIndex }, 'custom text'),
      reasoning: ({ partIndex }: { partIndex: number }) => h('span', { 'data-reasoning-index': partIndex }, 'custom reasoning'),
    })
    expect(html).toContain('data-text-index="0"')
    expect(html).toContain('data-reasoning-index="1"')
    expect(html).toContain('default notice')
    expect(html).not.toContain('default text')
    expect(html).not.toContain('default reasoning')
  })

  it('keeps an empty planning shell and its extension points visible', async () => {
    const shell = message('planning', 'assistant', 'gpt-5')
    shell.status = 'pending'
    shell.parts = []
    const html = await render(ChatMessagePanel, { messages: [shell], assistantActionsReady: false }, {
      'before-content': () => h('div', { 'data-planning-panel': '' }, 'planning'),
      'after-content': () => h('div', { 'data-result-files': '' }, 'files'),
    })
    expect(html).toContain('class="jcr-message__header"')
    expect(html).toContain('data-planning-panel')
    expect(html).toContain('class="jcr-message__content"')
    expect(html).toContain('data-result-files')
    expect(html).not.toContain('class="jcr-actions"')
    expect(html).not.toContain('jcr-fallback')
  })

  it('can hide assistant actions only for the active turn', async () => {
    const messages = [
      message('u1', 'user'), message('a1', 'assistant', 'gpt-5'),
      message('u2', 'user'), message('a2', 'assistant', 'gpt-5'),
    ]
    const html = await render(ChatMessagePanel, {
      messages,
      assistantActionsReady: (_message: ChatMessage, context: { turnId?: string }) => context.turnId !== 'turn-u2',
    })
    expect(html.match(/class="jcr-actions"/g)).toHaveLength(3)
    expect(html).toContain('data-message-id="a1"')
    expect(html).toContain('data-message-id="a2"')
    const firstAssistant = html.indexOf('data-message-id="a1"')
    const secondUser = html.indexOf('data-message-id="u2"')
    expect(html.slice(firstAssistant, secondUser)).toContain('class="jcr-actions"')
    expect(html.slice(html.indexOf('data-message-id="a2"'))).not.toContain('class="jcr-actions"')
  })

  it('forwards content overrides through grouped tool messages', async () => {
    const tool = (id: string): ChatMessage => ({
      protocolVersion: 1, id, role: 'tool', createdAt: 1, status: 'complete',
      parts: [{ type: 'tool-call', id, name: 'Read', status: 'complete' }],
    })
    const html = await render(ChatMessagePanel, { messages: [message('u1', 'user'), tool('t1'), tool('t2')] }, {
      content: ({ message: value }: { message: ChatMessage }) => h('span', { 'data-group-content': value.id }, value.id),
    })
    expect(html).toContain('data-group-content="t1"')
    expect(html).toContain('data-group-content="t2"')
    expect(html).not.toContain('class="jcr-tool"')
    expect(html.match(/class="jcr-avatar"/g)?.length).toBe(1)
  })

  it('keeps the markdown root and first wrapper DOM contract', () => {
    expect(chatMarkdownRendererSource).toMatch(/class="jcr-markdown[^>]*"[^>]*>\s*<div/)
  })

  it('keeps image-part previews content-sized and capped at 200px', () => {
    expect(chatStyleSource).toContain('.jcr-markdown--image { display:block; width:fit-content; max-width:min(100%,200px);')
    expect(chatStyleSource).toContain('max-width:200px!important')
    expect(chatStyleSource).toContain('max-height:200px!important')
  })

  it('maps conversation message slots to the panel content API', async () => {
    const value = message('a1', 'assistant', 'gpt-5')
    value.parts = [
      { type: 'text', text: 'text' },
      { type: 'reasoning', text: 'reasoning' },
    ]
    const contentHtml = await render(ChatConversation, { messages: [value], showComposer: false }, {
      'message-content': () => h('span', { 'data-conversation-content': '' }, 'whole'),
    })
    expect(contentHtml).toContain('data-conversation-content')
    expect(contentHtml).not.toContain('>text<')

    const partsHtml = await render(ChatConversation, { messages: [value], showComposer: false }, {
      'message-text': () => h('span', { 'data-conversation-text': '' }, 'host text'),
      'message-reasoning': () => h('span', { 'data-conversation-reasoning': '' }, 'host reasoning'),
    })
    expect(partsHtml).toContain('data-conversation-text')
    expect(partsHtml).toContain('data-conversation-reasoning')
  })

  it('forwards custom action menus through the conversation and panel', async () => {
    const value = message('a1', 'assistant', 'gpt-5')
    const html = await render(ChatConversation, {
      messages: [value], showComposer: false,
      actions: { export: { customMenu: true } },
      resolveActions: () => ['export'],
    }, {
      'action-menu': ({ action: resolved, placement }: { action: ResolvedMessageAction; placement: string }) =>
        h('span', { 'data-conversation-menu': resolved.name, 'data-placement': placement }, 'custom export'),
    })
    expect(html).toContain('data-conversation-menu="export"')
    expect(html).toContain('custom export')
  })
})

import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import {
  collectDeferredFastPlanMessageIds,
  createFastPlanThemeStyle,
  findFastPlanFinalAssistantMessageId,
  normalizeFastPlanSubmission,
  resolveFastPlanDownloadUrl,
  summarizeFastPlanProcessText,
  withFastPlanAssistantShell,
} from './conversation'
import fastPlanConversationSource from './FastPlanConversation.vue?raw'
import fastPlanTypesSource from './types.ts?raw'

const fastPlanStyleSource = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), 'style.css'), 'utf8')

describe('FastPlanConversation public contract', () => {
  it('uses the full available embedded and result width by default', () => {
    expect(fastPlanStyleSource).toContain('display:flex; width:100%; height:100%')
    expect(fastPlanStyleSource).toContain('.fastplan-conversation--embedded { display:block; width:100%;')
    expect(fastPlanStyleSource).toContain('.fastplan-conversation__result { box-sizing:border-box; width:100%; margin:.75rem 0;')
    expect(fastPlanStyleSource).toContain('.fastplan-conversation .jcr-message--user .jcr-message__main { width:auto; max-width:var(--jcr-bubble-max-width); }')
    expect(fastPlanStyleSource).toContain('@media (max-width:640px) { .fastplan-conversation .jcr-message--user .jcr-message__main { max-width:90%; }')
    expect(fastPlanStyleSource).not.toContain('960px')
    expect(fastPlanStyleSource).not.toContain('.fastplan-conversation .jcr-message--user .jcr-message__main { width:100%; max-width:none; }')
  })

  it('keeps model headers visible and only hides duplicate streaming status in compact density', () => {
    expect(fastPlanStyleSource).not.toMatch(/\.jcr-message:has\(\.fp-plan-card\) \.jcr-message__header/)
    expect(fastPlanStyleSource).toContain('.fastplan-conversation[data-density="compact"] .jcr-message:has(.fp-plan-card) .jcr-streaming { display:none; }')
    expect(fastPlanStyleSource).not.toContain('.fastplan-conversation .jcr-message:has(.fp-plan-card) .jcr-streaming { display:none; }')
  })

  it('resolves relative result files through the injected download base', () => {
    expect(resolveFastPlanDownloadUrl('/api/download/report.docx', 'https://files.example.com/')).toBe(
      'https://files.example.com/api/download/report.docx',
    )
    expect(resolveFastPlanDownloadUrl('https://cdn.example.com/report.docx', 'https://files.example.com')).toBe(
      'https://cdn.example.com/report.docx',
    )
  })

  it('normalizes string and structured composer submissions', () => {
    expect(normalizeFastPlanSubmission('  build a report  ')).toBe('build a report')
    expect(normalizeFastPlanSubmission({ text: '  shared composer event  ' })).toBe('shared composer event')
    expect(normalizeFastPlanSubmission({ value: '  continue  ' })).toBe('continue')
    expect(normalizeFastPlanSubmission({}, '  fallback  ')).toBe('fallback')
  })

  it('maps public theme tokens to shared CSS variables', () => {
    expect(createFastPlanThemeStyle({ background: '#101827', primary: '#7c3aed', radius: '18px', '--custom-token': '1' })).toEqual({
      '--jcr-bg': '#101827', '--jcr-primary': '#7c3aed', '--jcr-radius': '18px', '--custom-token': '1',
    })
  })

  it('adds a presentation-only assistant shell while an active plan has no answer', () => {
    const source = [{ id: 'user-1', role: 'user', createdAt: 1, parts: [] }]
    const messages = withFastPlanAssistantShell(source, { id: 'plan-1' }, { id: 'qwen', name: 'Qwen' })
    expect(source).toHaveLength(1)
    expect(messages).toHaveLength(2)
    expect(messages[1]).toMatchObject({
      id: 'temporary-fastplan-planning-user-1',
      role: 'assistant',
      status: 'pending',
      model: { id: 'qwen', name: 'Qwen' },
      parts: [],
      metadata: { fastPlanPlanningShell: true, anchorMessageId: 'user-1' },
    })
  })

  it('does not add a shell without a plan or when the turn already has an assistant answer', () => {
    const user = { id: 'user-1', role: 'user', createdAt: 1, parts: [] }
    const answered = [user, { id: 'assistant-1', role: 'assistant', createdAt: 2, parts: [] }]
    expect(withFastPlanAssistantShell([user], null, {})).toEqual([user])
    expect(withFastPlanAssistantShell(answered, { id: 'plan-1' }, {})).toBe(answered)
  })

  it('never treats the presentation-only shell as the final result message', () => {
    const shell = withFastPlanAssistantShell(
      [{ id: 'user-1', role: 'user', createdAt: 1, parts: [] }],
      { id: 'plan-1' },
      { id: 'qwen', name: 'Qwen' },
    )
    expect(findFastPlanFinalAssistantMessageId(shell)).toBeUndefined()

    const finalMessage = {
      id: 'assistant-final', role: 'assistant', parts: [],
      metadata: { contentKind: 'final_answer', fastPlanMessage: { isFinalAnswer: true } },
    }
    expect(findFastPlanFinalAssistantMessageId([...shell, finalMessage])).toBe('assistant-final')
  })

  it('finds restored final results marked by the message adapter', () => {
    expect(findFastPlanFinalAssistantMessageId([
      { id: 'assistant-summary', role: 'assistant', metadata: { fastPlanMessage: {} } },
      { id: 'assistant-restored', role: 'assistant', metadata: { contentKind: 'final_answer', completeFinalResult: true } },
    ])).toBe('assistant-restored')
  })

  it('defers only completed-turn process messages that precede a final answer', () => {
    const messages = [
      { id: 'u1', role: 'user' },
      { id: 'p1', role: 'assistant', status: 'complete', parts: [{ type: 'text', text: 'process' }], metadata: { contentKind: 'ordinary_message' } },
      { id: 'f1', role: 'assistant', status: 'complete', parts: [{ type: 'text', text: 'final' }], metadata: { contentKind: 'final_answer' } },
      { id: 'u2', role: 'user' },
      { id: 'p2', role: 'assistant', status: 'complete', parts: [{ type: 'text', text: 'still running' }], metadata: { contentKind: 'ordinary_message' } },
    ]
    expect([...collectDeferredFastPlanMessageIds(messages)]).toEqual(['p1'])
  })

  it('keeps errors visible and creates a bounded plain-text process summary', () => {
    const messages = [
      { id: 'u1', role: 'user' },
      { id: 'e1', role: 'assistant', status: 'error', parts: [{ type: 'text', text: 'failed' }], metadata: { contentKind: 'ordinary_message' } },
      { id: 'f1', role: 'assistant', status: 'complete', parts: [{ type: 'text', text: 'final' }], metadata: { contentKind: 'final_answer' } },
    ]
    expect(collectDeferredFastPlanMessageIds(messages).size).toBe(0)
    expect(summarizeFastPlanProcessText('## 阶段\n- **完成** [结果](https://example.com)', 20)).toBe('阶段 完成 结果')
    expect(summarizeFastPlanProcessText('a'.repeat(30), 10)).toBe('aaaaaaaaa…')
  })

  it('keeps planning and result panels on the public shell extension points', () => {
    const before = fastPlanConversationSource.indexOf('#message-before-content')
    const after = fastPlanConversationSource.indexOf('#message-after-content')
    const composer = fastPlanConversationSource.indexOf('#composer-before')
    expect(before).toBeGreaterThan(-1)
    expect(after).toBeGreaterThan(before)
    expect(composer).toBeGreaterThan(after)
    expect(fastPlanConversationSource).toContain(':assistant-actions-ready="assistantActionsReady"')
    expect(fastPlanConversationSource).toContain('return String(context.turnId) !== String(activeTurnId) || activeAssistantActionsReady.value')
    expect(fastPlanConversationSource).toContain('#action-menu')
    expect(fastPlanConversationSource).not.toContain('#message-content')
    expect(fastPlanConversationSource).toContain('#message-text')
    expect(fastPlanConversationSource).toContain(':theme="effectiveTheme"')
    expect(fastPlanConversationSource).toContain("message.metadata?.contentKind === 'final_answer'")
    expect(fastPlanConversationSource).not.toMatch(/completeFinalResult\s*\|\||isFinalAnswer\s*===/)
    expect(fastPlanConversationSource).not.toContain('#message-reasoning')
  })

  it('exposes the embedded host contract and structured result artifacts', () => {
    expect(fastPlanConversationSource).toContain("layout?: 'standalone' | 'embedded'")
    expect(fastPlanConversationSource).toContain("appearance?: 'default' | 'seamless'")
    expect(fastPlanConversationSource).toContain("composerMode?: 'internal' | 'host'")
    expect(fastPlanConversationSource).toContain('composerTarget?: string | HTMLElement | null')
    expect(fastPlanConversationSource).toContain('showTaskList?: boolean')
    expect(fastPlanConversationSource).toContain("props.composerMode === 'internal'")
    expect(fastPlanConversationSource).toContain('fastplan-conversation__host-composer-contribution')
    expect(fastPlanConversationSource).toContain("interactionPlacement?: 'composer' | 'inline'")
    expect(fastPlanConversationSource).toContain("errorPlacement?: 'top' | 'inline'")
    expect(fastPlanConversationSource).toContain(':show-composer="composerVisible"')
    expect(fastPlanConversationSource).toContain(':show-composer-auxiliary="false"')
    expect(fastPlanConversationSource).toContain(':scroll-mode="effectiveScrollMode"')
    expect(fastPlanConversationSource).toContain('connectionMode?:')
    expect(fastPlanConversationSource).toContain('sendChat, loadSession: runtime.loadSession')
    expect(fastPlanConversationSource).toContain('<FastPlanAnswer')
    expect(fastPlanConversationSource).not.toContain('v-for="(option, index) in result.charts"')
    expect(fastPlanConversationSource).toContain("confirmationPolicy?: 'legacy' | 'strict'")
    expect(fastPlanConversationSource).toContain('class="fastplan-conversation__inline-interaction"')
    expect(fastPlanConversationSource).toContain("confirmationModel.value?.kind === 'step'")
  })

  it('keeps forwarded conversation slots in the exported public types', () => {
    expect(fastPlanTypesSource).toContain('avatar(props: { message: ChatMessage })')
    expect(fastPlanTypesSource).toContain('header(props: { message: ChatMessage })')
    expect(fastPlanTypesSource).toContain("'action-icon'(props:")
    expect(fastPlanTypesSource).toContain("'action-menu'(props:")
    expect(fastPlanTypesSource).toContain("placement: 'direct' | 'overflow'")
  })
})

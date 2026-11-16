import { describe, expect, it } from 'vitest'
import { createFastPlanTimelinePlugin } from './timelinePlugin'
import rendererSource from './FastPlanTimelineRenderer.vue?raw'
import pluginSource from './timelinePlugin.ts?raw'

describe('FastPlan timeline plugin', () => {
  it('creates an explicit renderer registry without global installation', () => {
    const plugin = createFastPlanTimelinePlugin({ apiBase: '/fastPlan', executionMode: 'step' })
    expect(Object.keys(plugin.timelineRenderers)).toEqual(['fastplan'])
    expect(plugin.timelineRenderers.fastplan).toBeTruthy()
  })

  it('keeps all FastPlan network behavior inside planflow packages', () => {
    expect(rendererSource).toContain('createFastPlanClient')
    expect(rendererSource).toContain('uploadFastPlanFile')
    expect(rendererSource).toContain('<FastPlanConversation')
    expect(rendererSource).toContain("action('session-created'")
    expect(rendererSource).not.toContain('/aigc/')
    expect(rendererSource).not.toContain('$API')
  })

  it('normalizes protocol errors without coercing event objects to strings', () => {
    expect(rendererSource).toContain('normalizeFastPlanError(value)')
    expect(rendererSource).toContain('details: error.details ?? error.cause ?? value')
    expect(rendererSource).not.toContain("String(record.message || value || 'FastPlan 执行失败')")
  })

  it('uses embedded, seamless, host-scroll rendering with host-composer support and inline fallback', () => {
    expect(rendererSource).toContain('layout="embedded"')
    expect(rendererSource).toContain('appearance="seamless"')
    expect(rendererSource).toContain('interaction-placement="inline"')
    expect(rendererSource).toContain(':composer-mode="effectiveComposerMode"')
    expect(rendererSource).toContain(':composer-target="config.composerTarget"')
    expect(rendererSource).toContain(':show-task-list="effectiveShowTaskList"')
    expect(rendererSource).toContain(':density="config.density')
    expect(rendererSource).toContain(':compact-breakpoint="config.compactBreakpoint')
    expect(rendererSource).toContain("props.config.composerMode !== 'host'")
    expect(rendererSource).toContain('!!props.config.composerEntryId')
    expect(rendererSource).toContain('props.config.composerEntryId === props.entry.id')
    expect(rendererSource).toContain("props.config.composerMode !== 'host' || isComposerEntry.value")
    expect(rendererSource).not.toContain("effectiveConnectionMode.value === 'live'\n  && isComposerEntry.value")
    expect(rendererSource).toContain('const busy = ownsLiveComposer')
    expect(rendererSource).not.toContain("props.entry.status !== 'complete'")
    expect(rendererSource).not.toContain("props.entry.status !== 'failed'")
    expect(rendererSource).toContain('scroll-mode="host"')
    expect(rendererSource).toContain(':show-composer="false"')
    expect(pluginSource).toContain('config: { ...options }')
  })

  it('uses host-owned live runtimes and defers completed history trees', () => {
    expect(rendererSource).toContain('props.config.sessionManager')
    expect(rendererSource).toContain('manager.getOrCreate(sessionId')
    expect(rendererSource).toContain(':runtime="managedRuntime"')
    expect(rendererSource).toContain('historyDeferred')
    expect(rendererSource).toContain('historyExpanded = true')
  })
})

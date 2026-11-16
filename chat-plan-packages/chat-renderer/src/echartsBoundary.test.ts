import { readFileSync } from 'node:fs'
import { createSSRApp, h } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { expect, it, vi } from 'vitest'
import ChatMarkdownRenderer from './ChatMarkdownRenderer.vue'
import { splitFastPlanFinalAnswer } from '../../fastplan-runtime/src/answerParts'
import { MdPreview } from 'md-editor-v3'

// DOMPurify needs a browser window; keep the real MdPreview parser in this SSR regression.
vi.mock('dompurify', () => ({ default: { sanitize: (html: string) => html } }))

it('keeps the captured task chart as code while the final answer has one inline chart', async () => {
  const { nodeContent: content, finalContent } = JSON.parse(readFileSync(new URL('./fixtures/weather-chart.json', import.meta.url), 'utf8'))
  expect(content).toContain('```echarts')
  const defaultHtml = await renderToString(createSSRApp({ render: () => h(MdPreview, { modelValue: content }) }))
  expect(defaultHtml).toMatch(/class="[^"]*md-editor-echarts/)
  const html = await renderToString(createSSRApp({ render: () => h(ChatMarkdownRenderer, { content, maxChars: 100000 }) }))
  expect(html).toContain('<pre')
  expect(html).toContain('最高温')
  expect(html).not.toMatch(/class="[^"]*md-editor-echarts/)
  expect(html).not.toContain('<canvas')
  // The package-level plugin boundary must also protect restored/history views
  // if a caller accidentally creates MdPreview without the noEcharts prop.
  const afterPackageConfig = await renderToString(createSSRApp({ render: () => h(MdPreview, { modelValue: content }) }))
  expect(afterPackageConfig).toContain('<pre')
  expect(afterPackageConfig).not.toMatch(/class="[^"]*md-editor-echarts/)
  const parts = splitFastPlanFinalAnswer({ kind: 'final_answer', content: finalContent })
  expect(parts.filter(part => part.type === 'chart')).toHaveLength(1)
  expect(parts[0].type).toBe('text')
  expect(parts[1].type).toBe('chart')
})

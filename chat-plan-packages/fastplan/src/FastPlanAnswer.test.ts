import { createSSRApp, h } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { expect, it, vi } from 'vitest'

vi.mock('chat-renderer', () => ({
  ChatMarkdownRenderer: { props: ['content'], setup: (props: any) => () => h('section', props.content) },
}))
vi.mock('planflow-planning-ui', () => ({
  FastPlanEchartsBlock: { props: ['option', 'source', 'theme'], setup: (props: any) => () => h('figure', { 'data-theme': props.theme }, props.option.title.text) },
}))
import FastPlanAnswer from './FastPlanAnswer.vue'

it.each(['light', 'dark'] as const)('renders charts in order and forwards the %s theme', async theme => {
  const chart = (title: string) => '```echarts\n' + JSON.stringify({ title: { text: title }, series: [{ type: 'bar', data: [1] }] }) + '\n```\n'
  const content = 'Introduction\n\n' + chart('Chart A') + '\nAnalysis A\n\n' + chart('Chart B') + '\nConclusion'
  const html = await renderToString(createSSRApp({ render: () => h(FastPlanAnswer, { kind: 'final_answer', content, theme }) }))
  const positions = ['Introduction', `<figure data-theme="${theme}">Chart A</figure>`, 'Analysis A', `<figure data-theme="${theme}">Chart B</figure>`, 'Conclusion'].map(value => html.indexOf(value))
  expect(positions.every((position, index) => position >= 0 && (!index || position > positions[index - 1]))).toBe(true)
  expect(html).not.toContain('```echarts')
})

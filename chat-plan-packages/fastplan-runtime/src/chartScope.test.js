import { readFileSync } from 'node:fs'
import { expect, it } from 'vitest'

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8')

it('keeps every task/message compatibility component free of direct ECharts rendering', () => {
  const legacyTask = read('../../../components/blocks/TaskBlock.vue')
  const legacyMessage = read('../../../components/ChatMessage.vue')
  const activeTask = read('../../fastplan-planning-ui/src/components/blocks/TaskBlock.vue')
  for (const source of [legacyTask, legacyMessage, activeTask]) {
    expect(source).not.toContain("from 'echarts'")
    expect(source).not.toContain('echarts.init(')
    expect(source).not.toContain('extractEchartsBlocks')
    expect(source).not.toContain('outputCharts')
  }
})

it('does not expose recursive ECharts extraction APIs outside final-answer parsing', () => {
  const legacyArtifacts = read('../../../utils/mdAnswer.js')
  const publicRuntimeApi = read('./index.ts')
  const forbiddenApis = [
    'extractEchartsOptions',
    'tryParseEchartsOption',
    'stripEchartsFromText',
    'extractFastPlanEchartsOptions',
    'isFastPlanEchartsOption',
    'stripFastPlanEchartsFromText',
  ]

  for (const api of forbiddenApis) {
    expect(legacyArtifacts).not.toContain(api)
    expect(publicRuntimeApi).not.toContain(api)
  }
})

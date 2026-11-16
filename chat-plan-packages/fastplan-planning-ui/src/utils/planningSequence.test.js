import { describe, expect, it } from 'vitest'
import { sequencePlanningRows } from './planningSequence'

describe('planning stage display sequence', () => {
  it('buffers retrieval when it arrives before intent', () => {
    const rows = sequencePlanningRows([
      { stage: 'retrieval', status: 'done', content: '5 mcp · 3 agent' },
    ], { planningActive: true })

    expect(rows).toEqual([expect.objectContaining({ stage: 'intent', status: 'running', synthetic: true })])
  })

  it('shows only intent while intent is running', () => {
    const intent = { stage: 'intent', status: 'running', content: '正在分析意图' }
    expect(sequencePlanningRows([
      intent,
      { stage: 'retrieval', status: 'done', content: '5 mcp · 3 agent' },
      { stage: 'capability', status: 'done', content: '2 项能力' },
    ], { planningActive: true })).toEqual([intent])
  })

  it('releases retrieval and later stages after intent completes', () => {
    const rows = [
      { stage: 'intent', status: 'done', content: '意图已确认' },
      { stage: 'retrieval', status: 'done', content: '5 mcp · 3 agent' },
      { stage: 'capability', status: 'done', content: '2 项能力' },
    ]
    expect(sequencePlanningRows(rows, { planningActive: true })).toEqual(rows)
  })

  it('does not invent an intent stage for completed non-planning history', () => {
    const retrieval = { stage: 'retrieval', status: 'done', content: '5 mcp · 3 agent' }
    expect(sequencePlanningRows([retrieval], { planningActive: false })).toEqual([retrieval])
  })
})

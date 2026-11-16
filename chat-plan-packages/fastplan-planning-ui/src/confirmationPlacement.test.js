// @vitest-environment jsdom
import { shallowMount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import PlanPreview from './components/PlanPreview.vue'

const tree = { root: { children: [{ id: 'phase-1', name: '查询与整合', children: [
  { id: 'query', name: '查询天气', node_type: { type: 'ToolTask', config: {} } },
  { id: 'merge', name: '整合天气', node_type: { type: 'LlmTask', config: {} } },
] }] } }

describe('step confirmation placement', () => {
  it('keeps planned tasks and shows a concise terminal status after cancellation', () => {
    const wrapper = shallowMount(PlanPreview, { props: {
      tree,
      phase: 'finished',
      executionStatus: 'cancelled',
      activeView: 'list',
      executionState: {},
    } })

    expect(wrapper.text()).not.toContain('等待任务开始执行')
    expect(wrapper.get('[data-plan-cancelled-status]').text()).toBe('任务已取消执行')
    expect(wrapper.findAll('task-block-stub')).toHaveLength(2)
    wrapper.unmount()
  })

  it('renders each confirmation immediately before its corresponding task card', () => {
    const wrapper = shallowMount(PlanPreview, { props: {
      tree,
      phase: 'executing',
      activeView: 'list',
      executionState: {
        query: { id: 'query', name: '查询天气', status: 'completed' },
        merge: { id: 'merge', name: '整合天气', status: 'completed' },
      },
      confirmedHistory: [
        { id: 'review-query', nodeId: 'query', completedNodeName: '查询天气', timestamp: new Date() },
        { id: 'review-merge', nodeId: 'merge', completedNodeName: '整合天气', timestamp: new Date() },
      ],
    } })

    const order = Array.from(wrapper.element.querySelectorAll('manual-review-panel-stub, task-block-stub'))
      .map(element => element.tagName.toLowerCase())
    expect(order).toEqual([
      'manual-review-panel-stub',
      'task-block-stub',
      'manual-review-panel-stub',
      'task-block-stub',
    ])
    wrapper.unmount()
  })
})

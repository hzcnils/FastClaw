// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FastPlanTaskList from './FastPlanTaskList.vue'

const plan: any = {
  id: 'tree-1', status: 'paused', phases: [{ id: 'phase-1', title: '参数确认', nodes: [] }],
}

describe('FastPlanTaskList structured checkpoint', () => {
  it('exposes the resolved density on its rendered root', () => {
    const wrapper = mount(FastPlanTaskList, { props: { plan, density: 'compact' } })
    expect(wrapper.get('.jcr-plan-tasks').attributes('data-density')).toBe('compact')
    wrapper.unmount()
  })

  it('collects required input instead of showing continue/cancel choices', async () => {
    const wrapper = mount(FastPlanTaskList, { props: {
      plan,
      confirmation: {
        id: 'node-1', kind: 'human', interactionKind: 'input', title: '需要补充信息',
        question: '请提供地区，以便后续查询和绘图', options: [],
        fields: [{ key: 'region', label: '地区', type: 'text', required: true, placeholder: '例如：浙江省' }],
        submitLabel: '提交并继续', cancelLabel: '取消任务',
      },
    } })

    expect(wrapper.text()).toContain('请填写：地区')
    expect(wrapper.text()).not.toContain('继续执行')
    expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined()
    await wrapper.get('input').setValue('浙江省')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.emitted('confirmation-select')?.[0]?.[0]).toMatchObject({
      action: 'submit', values: { region: '浙江省' },
    })
    wrapper.unmount()
  })
})

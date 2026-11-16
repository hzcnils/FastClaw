// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ChatParameterForm from './ChatParameterForm.vue'

describe('ChatParameterForm', () => {
  it('renders arbitrary fields, masks secrets and blocks blank required values', async () => {
    const wrapper = mount(ChatParameterForm, { props: {
      fields: [{ key: 'endpoint', label: 'Endpoint', required: true }, { key: 'secret', label: 'Secret', type: 'password', required: true }],
      modelValue: { endpoint: '   ', secret: 'key' },
    } })
    expect(wrapper.find('input[type=password]').exists()).toBe(true)
    await wrapper.find('form').trigger('submit')
    expect(wrapper.emitted('submit')).toBeUndefined()
    await wrapper.find('input').setValue('https://example.com')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([{ endpoint: 'https://example.com', secret: 'key' }])
    await wrapper.setProps({ modelValue: { endpoint: ' https://example.com ', secret: ' key ' } })
    await wrapper.find('form').trigger('submit')
    expect(wrapper.emitted('submit')?.[0]).toEqual([{ endpoint: 'https://example.com', secret: 'key' }])
  })

  it('exposes save errors and prevents duplicate submissions while saving', async () => {
    const wrapper = mount(ChatParameterForm, { props: {
      fields: [{ key: 'value', label: 'Value' }], modelValue: { value: 'value' },
      submitting: true, error: 'Save failed',
    } })
    await wrapper.find('form').trigger('submit')
    expect(wrapper.emitted('submit')).toBeUndefined()
    expect(wrapper.find('[role=alert]').text()).toBe('Save failed')
    expect(wrapper.find('input').attributes('disabled')).toBeDefined()
    await wrapper.setProps({ submitting: false })
    await wrapper.find('button[type=button]').trigger('click')
    expect(wrapper.emitted('cancel')).toHaveLength(1)
  })
})

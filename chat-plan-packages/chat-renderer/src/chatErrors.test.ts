// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ChatMessageItem from './ChatMessageItem.vue'
import { formatChatErrorRaw, resolveChatError } from './chatErrors'
import { defaultMessages } from './messages'
import type { ChatMessage, ResolvedMessageAction } from './types'

describe('resolveChatError', () => {
  it.each([
    ['HTTP状态码:401\n响应: Authentication Fails (governor)', 'authentication', '请确认当前配置的密钥是否正确'],
    ['Invalid token', 'authentication', '请确认当前配置的密钥是否正确'],
    ['HTTP 502 Bad Gateway', 'gateway', '模型服务暂时无法访问，请确认接口地址及网络是否可访问'],
    ['HTTP status code: 503', 'model-unavailable', '当前模型不可用，请重新检查模型信息'],
    ['{"code":502,"message":"Bad Gateway"}', 'gateway', '模型服务暂时无法访问，请确认接口地址及网络是否可访问'],
  ])('classifies %s', (raw, category, message) => {
    expect(resolveChatError(raw)).toMatchObject({ category, message })
  })

  it('classifies quota exhaustion without a payment action', () => {
    const quota = resolveChatError('HTTP状态码:403\n用户额度不足')
    expect(quota.category).toBe('quota')
    expect(quota.message).toBe('当前模型账户额度或余额不足')
    expect(quota.actions).toEqual([])

    const permission = resolveChatError('HTTP状态码:403\npermission denied')
    expect(permission).toMatchObject({ category: 'permission', message: '当前密钥没有访问该模型或接口的权限' })
    expect(permission.actions).toEqual([])
  })

  it('distinguishes quota exhaustion from ordinary 429 rate limiting', () => {
    expect(resolveChatError('HTTP 429 quota exceeded').category).toBe('quota')
    expect(resolveChatError('HTTP 429 too many requests')).toMatchObject({ category: 'rate-limit', actions: [] })
  })

  it('recognizes reqwest connection interruption chains as network errors', () => {
    const raw = 'error sending request for url (...) -> client error (SendRequest) -> connection closed before message completed'
    expect(resolveChatError(raw)).toMatchObject({
      category: 'network',
      message: '无法访问模型服务，请检查网络和接口地址',
    })
  })

  it('formats legacy one-line request failures like structured HTTP errors', () => {
    const raw = '错误: LLM 调用错误: 请求 http://192.0.2.10:3000/v1/chat/completions 失败（模型 glm-5.1 / 私域网）: error sending request'
    expect(formatChatErrorRaw(raw)).toBe([
      '错误: LLM 调用错误: LLM 请求错误',
      '模型: glm-5.1',
      '供应商：私域网 (http://192.0.2.10:3000/v1)',
      '请求地址:http://192.0.2.10:3000/v1/chat/completions',
      '错误详情:',
      'error sending request',
    ].join('\n'))
  })

  it('honors structured host overrides', () => {
    expect(resolveChatError({
      raw: 'opaque provider response', statusCode: 403, category: 'account-restricted',
      message: 'Custom host message', actions: [{ id: 'support', label: 'Support' }],
    })).toMatchObject({ category: 'account-restricted', message: 'Custom host message', actions: [{ id: 'support' }] })
  })
})

describe('built-in message error UI', () => {
  it('hides standard actions for quota errors', () => {
    const message: ChatMessage = {
      id: 'error-1', role: 'assistant', createdAt: 1, status: 'error',
      error: { raw: 'HTTP状态码:403\ninsufficient_user_quota' },
      parts: [{ type: 'text', text: 'must not render twice' }],
    }
    const actions: ResolvedMessageAction[] = [{
      name: 'copy', actionId: 'copy', label: 'Copy', order: 0, visible: true, disabled: false,
    }]
    const wrapper = mount(ChatMessageItem, { props: { message, actions, messages: defaultMessages } })

    expect(wrapper.find('.jcr-actions').exists()).toBe(false)
    expect(wrapper.find('.jcr-message-error__action').exists()).toBe(false)
    expect(wrapper.text()).toContain('当前模型账户额度或余额不足')
    expect(wrapper.text()).not.toContain('must not render twice')
  })

  it('renders compact retry progress and suppresses ordinary streaming content', () => {
    const message: ChatMessage = {
      id: 'retry-1', role: 'assistant', createdAt: 1, status: 'streaming',
      retry: { attempt: 2, total: 5 },
      parts: [{ type: 'text', text: 'temporary content' }],
    }
    const wrapper = mount(ChatMessageItem, { props: { message, messages: defaultMessages } })
    expect(wrapper.get('.jcr-message-retry').text()).toBe('正在重试中…2/5')
    expect(wrapper.text()).not.toContain('temporary content')
    expect(wrapper.find('.jcr-streaming').exists()).toBe(false)
  })
})

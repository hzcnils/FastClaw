import { describe, expect, it } from 'vitest'
import { resolveFastPlanServiceConfig, toFastPlanWebSocketBase } from './serviceConfig'

describe('FastPlan service configuration', () => {
  it('derives ws from a direct http service', () => {
    expect(resolveFastPlanServiceConfig('http://127.0.0.1:8080/')).toEqual({
      apiBase: 'http://127.0.0.1:8080',
      wsBase: 'ws://127.0.0.1:8080',
      uploadBase: 'http://127.0.0.1:8080',
      downloadBase: 'http://127.0.0.1:8080',
      adminBase: 'http://127.0.0.1:8080',
    })
  })

  it('uses wss for an https service', () => {
    expect(toFastPlanWebSocketBase('https://fastplan.example.com/')).toBe('wss://fastplan.example.com')
  })

  it('resolves a relative proxy path only when an origin is supplied', () => {
    expect(toFastPlanWebSocketBase('/FastPlan/', { protocol: 'https:', host: 'app.example.com' }))
      .toBe('wss://app.example.com/FastPlan')
  })

  it('keeps an explicit websocket base unchanged', () => {
    expect(toFastPlanWebSocketBase('ws://127.0.0.1:8080/')).toBe('ws://127.0.0.1:8080')
  })
})

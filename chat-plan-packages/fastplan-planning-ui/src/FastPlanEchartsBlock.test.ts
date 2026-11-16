// @vitest-environment jsdom
import { createApp, h, nextTick, reactive } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import FastPlanEchartsBlock from './FastPlanEchartsBlock.vue'

const mocks = vi.hoisted(() => ({ init: vi.fn(), use: vi.fn(), version: '6.0.0' }))
vi.mock('echarts/core', () => mocks)
const mounted: Array<() => void> = []
afterEach(() => { mounted.splice(0).forEach(stop => stop()); vi.restoreAllMocks(); mocks.init.mockReset(); mocks.version = '6.0.0' })
const settle = async () => { await nextTick(); await nextTick() }
const instance = (modern = true) => ({ setOption: vi.fn(), dispose: vi.fn(), resize: vi.fn(), ...(modern ? { setTheme: vi.fn() } : {}) })
function mount(theme = 'light') {
  const props = reactive({ theme, option: { backgroundColor: '#fff', series: [{ type: 'bar', data: [1], itemStyle: { color: '#f00' } }] } })
  const root = document.createElement('div')
  document.body.append(root)
  const app = createApp({ render: () => h(FastPlanEchartsBlock, props) })
  app.mount(root)
  mounted.push(() => { app.unmount(); root.remove() })
  return { props, root, app }
}
it('switches light/dark on modern instances without rewriting explicit option colors', async () => {
  const chart = instance()
  mocks.init.mockReturnValue(chart)
  const { props, root } = mount()
  await settle()
  expect(mocks.init.mock.calls[0][1]).toBeUndefined()
  const original = JSON.stringify(props.option)
  props.theme = 'dark'; await settle()
  expect(chart.setTheme).toHaveBeenLastCalledWith('dark')
  expect(root.firstElementChild?.getAttribute('data-theme')).toBe('dark')
  props.theme = 'light'; await settle()
  expect(chart.setTheme).toHaveBeenLastCalledWith('default')
  expect(mocks.init).toHaveBeenCalledTimes(1)
  expect(chart.setOption).toHaveBeenCalledTimes(1)
  expect(JSON.stringify(props.option)).toBe(original)
})
it('recreates ECharts 5 even when its deprecated setTheme method exists', async () => {
  mocks.version = '5.6.0'
  const first = instance(), second = instance()
  mocks.init.mockReturnValueOnce(first).mockReturnValueOnce(second)
  const add = vi.spyOn(window, 'addEventListener'), remove = vi.spyOn(window, 'removeEventListener')
  const { props, app } = mount('dark')
  await settle()
  expect(mocks.init.mock.calls[0][1]).toBe('dark')
  props.theme = 'light'; await settle()
  expect(first.setTheme).not.toHaveBeenCalled()
  expect(first.dispose).toHaveBeenCalledOnce()
  expect(mocks.init.mock.calls[1][1]).toBeUndefined()
  expect(second.setOption).toHaveBeenCalledWith(props.option, true)
  expect(add.mock.calls.filter(call => call[0] === 'resize')).toHaveLength(1)
  window.dispatchEvent(new Event('resize'))
  expect(second.resize).toHaveBeenCalledOnce()
  app.unmount()
  expect(second.dispose).toHaveBeenCalledOnce()
  expect(remove.mock.calls.filter(call => call[0] === 'resize')).toHaveLength(1)
})

it('uses the recreation path when the ECharts version cannot be identified', async () => {
  mocks.version = ''
  const first = instance(), second = instance()
  mocks.init.mockReturnValueOnce(first).mockReturnValueOnce(second)
  const { props } = mount()
  await settle()
  props.theme = 'dark'; await settle()
  expect(first.setTheme).not.toHaveBeenCalled()
  expect(first.dispose).toHaveBeenCalledOnce()
  expect(mocks.init.mock.calls[1][1]).toBe('dark')
})

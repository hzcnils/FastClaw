<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as echarts from 'echarts/core'
import { BarChart, FunnelChart, GaugeChart, GraphChart, HeatmapChart, LineChart, PieChart, RadarChart, ScatterChart } from 'echarts/charts'
import { CalendarComponent, DataZoomComponent, DatasetComponent, GeoComponent, GridComponent, LegendComponent, PolarComponent, RadarComponent, TitleComponent, ToolboxComponent, TooltipComponent, TransformComponent, VisualMapComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'

echarts.use([
  BarChart, FunnelChart, GaugeChart, GraphChart, HeatmapChart, LineChart, PieChart, RadarChart, ScatterChart,
  CalendarComponent, DataZoomComponent, DatasetComponent, GeoComponent, GridComponent, LegendComponent,
  PolarComponent, RadarComponent, TitleComponent, ToolboxComponent, TooltipComponent, TransformComponent,
  VisualMapComponent, CanvasRenderer,
])

const props = defineProps({
  option: { type: Object, required: true },
  height: { type: String, default: '340px' },
  source: { type: String, default: '' },
  theme: { type: String, default: 'light', validator: value => ['light', 'dark'].includes(value) },
})
const failed = ref(false)
const chartElement = ref(null)
let chart = null
let resizeObserver = null

function resize() { chart?.resize() }
function getEchartsMajorVersion() {
  const major = Number.parseInt(String(echarts.version || '').split('.')[0], 10)
  return Number.isFinite(major) ? major : 0
}
function render() {
  if (!chartElement.value || !props.option) return
  try {
    if (!chart) {
      chart = echarts.init(chartElement.value, props.theme === 'dark' ? 'dark' : undefined)
    }
    chart.setOption(props.option, true)
    failed.value = false
  } catch {
    failed.value = true
  }
}

watch(() => props.option, () => nextTick(render), { deep: true })
watch(() => props.theme, async () => {
  await nextTick()
  if (!chart) return render()
  try {
    if (getEchartsMajorVersion() >= 6 && typeof chart.setTheme === 'function') {
      chart.setTheme(props.theme === 'dark' ? 'dark' : 'default')
      failed.value = false
    } else {
      // ECharts 5 keeps a deprecated no-op setTheme method. Recreate the chart
      // and reapply the original option rather than getOption(), which includes
      // defaults from the previous theme. Unknown versions use the safe path too.
      chart.dispose()
      chart = null
      render()
    }
  } catch {
    failed.value = true
  }
})
onMounted(() => {
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(resize)
    resizeObserver.observe(chartElement.value)
  }
  if (typeof window !== 'undefined') window.addEventListener('resize', resize)
  nextTick(render)
})
onBeforeUnmount(() => {
  if (typeof window !== 'undefined') window.removeEventListener('resize', resize)
  resizeObserver?.disconnect()
  chart?.dispose()
  chart = null
})
</script>

<template>
  <div class="fpp-echarts-block" :data-theme="theme">
    <pre v-if="failed" style="white-space:pre-wrap;overflow-wrap:anywhere">{{ source || JSON.stringify(option, null, 2) }}</pre>
    <div v-show="!failed" ref="chartElement" class="fpp-echarts-block__canvas" :style="{ height }" />
  </div>
</template>

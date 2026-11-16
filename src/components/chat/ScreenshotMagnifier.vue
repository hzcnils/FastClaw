<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'

const props = defineProps<{
  mouseX: number       // CSS 像素（相对于 overlay 左上角）
  mouseY: number
  imageDataUrl: string
  zoom?: number        // 默认 4
  imageWidth: number
  imageHeight: number
  scaleX: number       // 1/displayScale：CSS px → 图像 px
  scaleY: number
  offsetX: number
  offsetY: number
}>()

const MAG_W = 130
const MAG_H = 210
const IMG_H = 150   // 图像区域高度
const INFO_H = MAG_H - IMG_H  // 信息区域高度（60px）
const RADIUS = 8

const zoom = computed(() => props.zoom ?? 4)
const canvasRef = ref<HTMLCanvasElement>()
let imgEl: HTMLImageElement | null = null
let imgLoaded = false

// RAF 节流：鼠标可达 1000Hz，而 draw() 内含 getImageData（强制 GPU 同步回读，
// 每次都很贵）。若绑在原始 mousemove 上会每秒回读上千次，造成明显卡顿。
// 这里把绘制合并到每帧最多一次。
let rafPending = false
function scheduleDraw() {
  if (rafPending) return
  rafPending = true
  requestAnimationFrame(() => {
    rafPending = false
    draw()
  })
}

const pixelHex = ref('#000000')
const coordX = computed(() => Math.round((props.mouseX - props.offsetX) * props.scaleX))
const coordY = computed(() => Math.round((props.mouseY - props.offsetY) * props.scaleY))

function loadImage() {
  if (!props.imageDataUrl) return
  imgEl = new Image()
  imgEl.onload = () => { imgLoaded = true; draw() }
  imgEl.src = props.imageDataUrl
}

function draw() {
  const canvas = canvasRef.value
  if (!canvas || !imgEl || !imgLoaded) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const z = zoom.value
  const imgX = coordX.value
  const imgY = coordY.value

  const srcW = MAG_W / z
  const srcH = IMG_H / z
  const srcX = imgX - srcW / 2
  const srcY = imgY - srcH / 2

  canvas.width = MAG_W
  canvas.height = IMG_H

  ctx.clearRect(0, 0, MAG_W, IMG_H)
  ctx.drawImage(imgEl, srcX, srcY, srcW, srcH, 0, 0, MAG_W, IMG_H)

  // 读取中心像素颜色（在十字线绘制之前）
  const px = ctx.getImageData(MAG_W / 2, IMG_H / 2, 1, 1).data
  pixelHex.value = '#' + [px[0], px[1], px[2]]
    .map(v => v.toString(16).padStart(2, '0')).join('').toUpperCase()

  // 十字线
  ctx.strokeStyle = '#00e676'
  ctx.lineWidth = 1
  ctx.beginPath()
  ctx.moveTo(MAG_W / 2, 0); ctx.lineTo(MAG_W / 2, IMG_H)
  ctx.moveTo(0, IMG_H / 2); ctx.lineTo(MAG_W, IMG_H / 2)
  ctx.stroke()
}

watch(() => [props.mouseX, props.mouseY], scheduleDraw)
watch(() => props.imageDataUrl, () => { imgLoaded = false; loadImage() })

onMounted(() => loadImage())
onUnmounted(() => {
  imgEl = null
  rafPending = false
})

// 偏移到光标右下角，靠近屏幕边缘时自动翻转
const wrapperStyle = computed(() => {
  const GAP = 14
  let left = props.mouseX + GAP
  let top = props.mouseY + GAP
  // 右侧放不下时翻到左侧
  if (left + MAG_W > window.innerWidth - 4) left = props.mouseX - MAG_W - GAP
  // 底部放不下时翻到上侧
  if (top + MAG_H > window.innerHeight - 4) top = props.mouseY - MAG_H - GAP
  return {
    position: 'absolute' as const,
    left: left + 'px',
    top: top + 'px',
    width: MAG_W + 'px',
    height: MAG_H + 'px',
    borderRadius: RADIUS + 'px',
    overflow: 'hidden',
    border: '1.5px solid rgba(75,107,252,0.85)',
    boxSizing: 'border-box' as const,
    pointerEvents: 'none' as const,
    zIndex: 50,
    boxShadow: '0 4px 20px rgba(0,0,0,0.5)',
  }
})
</script>

<template>
  <div :style="wrapperStyle">
    <canvas ref="canvasRef" :width="MAG_W" :height="IMG_H" style="display:block" />
    <div class="mag-info" :style="{ height: INFO_H + 'px' }">
      <div class="mag-row">
        <span class="mag-label">坐标</span>
        <span class="mag-value">{{ coordX }}, {{ coordY }}</span>
      </div>
      <div class="mag-row">
        <span class="mag-label">色值</span>
        <span class="mag-value">
          <span class="color-swatch" :style="{ background: pixelHex }" />
          {{ pixelHex }}
        </span>
      </div>
      <div class="mag-hint">Ctrl+C 复制色值</div>
    </div>
  </div>
</template>

<style scoped>
.mag-info {
  background: rgba(18, 18, 30, 0.96);
  padding: 7px 10px 5px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
}

.mag-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
  font-family: monospace;
  color: #fff;
}

.mag-label {
  color: rgba(255, 255, 255, 0.45);
  flex-shrink: 0;
  margin-right: 6px;
}

.mag-value {
  display: flex;
  align-items: center;
  gap: 5px;
  font-weight: 500;
  letter-spacing: 0.5px;
}

.color-swatch {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 2px;
  border: 1px solid rgba(255,255,255,0.25);
  flex-shrink: 0;
}

.mag-hint {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.28);
  text-align: center;
  margin-top: 1px;
  font-family: monospace;
}
</style>

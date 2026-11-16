// FastClaw · 独立截图遮罩窗口入口
// 专用于 `screenshot` 窗口：只挂载遮罩承载页，不加载主应用外壳
//（无 Pinia / Vue Router / Element Plus / i18n），弹出快、无副作用、不影响主窗口。
import { createApp } from 'vue'
import ScreenshotWindow from './views/ScreenshotWindow.vue'

createApp(ScreenshotWindow).mount('#screenshot-app')

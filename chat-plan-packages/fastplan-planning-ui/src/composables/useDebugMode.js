// 全局「简洁 / 调试」模式（对照 v21 原型右上角双态切换）
// 模块级单例 ref，各组件直接 import 即共享同一状态。
import { ref } from 'vue'

export const debugMode = ref('simple') // 'simple' | 'debug'

export function toggleDebugMode() {
  debugMode.value = debugMode.value === 'simple' ? 'debug' : 'simple'
}

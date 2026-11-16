// FastClaw · 右侧网页预览面板状态（搜索结果点外链 → fetch_web_page → iframe）
//
// 由 SearchResultsContent 在用户点击折叠列表标题时驱动：
//   openExternalLoading → 抓取中（骨架屏）→ openExternal / failExternal
// 面板本身由 ChatView 的右侧 aside 渲染（rightPanel === 'preview'），
// 显隐以本 store 的 isOpen 为唯一事实来源。
import { defineStore } from 'pinia'
import { ref } from 'vue'

/** fetch_web_page 抓取成功后注入面板的载荷 */
export interface ExternalPreviewPayload {
  final_url: string
  /** 抓回来的原始 HTML（可能为空，表示非 HTML 内容） */
  html: string
  /** 目标页是否允许 iframe 嵌入 */
  embeddable: boolean
  /** 展示用标题 */
  title: string
  /** 阅读器模式净化后的正文 HTML（前端提取，embeddable=false 时兜底） */
  readerHtml: string
}

export const usePreviewPanelStore = defineStore('previewPanel', () => {
  const isOpen = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** 当前正在预览的 URL（结果列表高亮当前项） */
  const activeUrl = ref('')
  const externalTitle = ref('')
  const external = ref<ExternalPreviewPayload | null>(null)
  /** false=原始 iframe 直嵌；true=阅读器 srcdoc */
  const readerMode = ref(false)
  /** 自增触发 iframe 重挂载 */
  const reloadKey = ref(0)

  /** 抓取中：先开面板 + 转圈，抓完再注入内容 */
  function openExternalLoading(url: string, title: string) {
    isOpen.value = true
    loading.value = true
    error.value = null
    activeUrl.value = url
    externalTitle.value = title || url
    external.value = null
    readerMode.value = false
    reloadKey.value++
  }

  /** 抓取成功：注入内容（面板已开时复用，不叠加） */
  function openExternal(payload: ExternalPreviewPayload) {
    isOpen.value = true
    loading.value = false
    error.value = null
    external.value = payload
    activeUrl.value = payload.final_url || activeUrl.value
    externalTitle.value = payload.title || payload.final_url || externalTitle.value
    readerMode.value = !payload.embeddable
    reloadKey.value++
  }

  /** 抓取失败 / 非 HTML：面板内给出友好提示 + 系统浏览器兜底 */
  function failExternal(message: string) {
    loading.value = false
    error.value = message
    external.value = null
  }

  function setReaderMode(on: boolean) {
    readerMode.value = on
  }

  function close() {
    isOpen.value = false
    loading.value = false
    error.value = null
    activeUrl.value = ''
    externalTitle.value = ''
    external.value = null
    readerMode.value = false
    reloadKey.value++
  }

  return {
    isOpen,
    loading,
    error,
    activeUrl,
    externalTitle,
    external,
    readerMode,
    reloadKey,
    openExternalLoading,
    openExternal,
    failExternal,
    setReaderMode,
    close,
  }
})

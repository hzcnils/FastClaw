// FastClaw · 全局应用状态
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

export const useAppStore = defineStore('app', () => {
  const theme = ref<'light' | 'dark' | 'system'>('system')
  const sidebarCollapsed = ref(false)
  const currentMode = ref<'normal' | 'planning'>('normal')
  const showSettings = ref(false)

  // ─── Tab 标题中转（跨组件改名通道：ChatView → TabBar 非父子关系）───
  const tabTitles = ref<Record<string, string>>({})
  function setTabTitle(path: string, title: string) {
    tabTitles.value[path] = title
  }

  // ─── 空白「新建对话」tab 注册表 ───
  // 各 ChatView 实例在「当前会话为空」时登记自己的路由 path，
  // newCreateChat 借此判断 tab 栏是否已有可复用的空白对话 tab，避免重复创建空 tab。
  const blankChatTabs = ref<string[]>([])
  function registerBlankChatTab(path: string) {
    if (!blankChatTabs.value.includes(path)) blankChatTabs.value.push(path)
  }
  function unregisterBlankChatTab(path: string) {
    const i = blankChatTabs.value.indexOf(path)
    if (i >= 0) blankChatTabs.value.splice(i, 1)
  }
  function findBlankChatTab(): string | undefined {
    return blankChatTabs.value.find(p => p === '/chat' || p.startsWith('/chat/'))
  }

  // ─── 未完成任务标记（组件本地状态上报）───
  // path → reason → active
  const unfinishedFlags = ref<Record<string, Record<string, boolean>>>({})
  function setTabUnfinished(path: string, reason: string, active: boolean) {
    const prev = unfinishedFlags.value[path] || {}
    if (active) {
      unfinishedFlags.value = {
        ...unfinishedFlags.value,
        [path]: { ...prev, [reason]: true },
      }
      return
    }
    const next = { ...prev }
    delete next[reason]
    if (Object.keys(next).length === 0) {
      const all = { ...unfinishedFlags.value }
      delete all[path]
      unfinishedFlags.value = all
    } else {
      unfinishedFlags.value = { ...unfinishedFlags.value, [path]: next }
    }
  }
  const unfinishedFlagPaths = computed(() =>
    Object.keys(unfinishedFlags.value).filter(p =>
      Object.values(unfinishedFlags.value[p] || {}).some(Boolean),
    ),
  )

  function toggleTheme() { theme.value = theme.value === 'dark' ? 'light' : 'dark' }
  function toggleSidebar() { sidebarCollapsed.value = !sidebarCollapsed.value }
  function toggleSettings() { showSettings.value = !showSettings.value }

  return {
    theme, sidebarCollapsed, currentMode, showSettings, tabTitles, setTabTitle,
    blankChatTabs, registerBlankChatTab, unregisterBlankChatTab, findBlankChatTab,
    unfinishedFlags, setTabUnfinished, unfinishedFlagPaths,
    toggleTheme, toggleSidebar, toggleSettings,
  }
})

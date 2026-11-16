// FastClaw · Slash command system for chat input
// Typing "/" at line start shows available skills + MCP servers in a dropdown.

import { ref, computed, onUnmounted } from 'vue'
import type { Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface SlashCommand {
  name: string        // unique identifier (skill slug or mcp server id)
  labelEn: string     // English display name
  labelZh: string     // Chinese display name
  description: string // description / additional info
  type: 'skill' | 'mcp' | 'builtin'
}

interface UseSlashCommandsOptions {
  input: Ref<string>
  setInput: (value: string) => void
  textareaRef: Ref<any>
  onExecuteCommand?: (cmd: SlashCommand) => void
  /** 按钮模式选中回调（不插入文本，直接把选中项返回给调用方） */
  onButtonSelect?: (cmd: SlashCommand) => void
}

export function useSlashCommands({
  input,
  setInput,
  textareaRef,
  onExecuteCommand,
  onButtonSelect,
}: UseSlashCommandsOptions) {
  const allCommands = ref<SlashCommand[]>([])
  const showMenu = ref(false)
  const query = ref('')
  const searchQuery = ref('')
  const activeTab = ref<'all' | 'skill' | 'mcp'>('all')
  const selectedIndex = ref(-1)
  const slashPos = ref(-1)
  const loaded = ref(false)
  const buttonMode = ref(false)   // 按钮触发的弹出模式

  const filtered = computed(() => {
    let cmds = allCommands.value

    // Tab 类型过滤
    if (activeTab.value !== 'all') {
      cmds = cmds.filter(c => c.type === activeTab.value)
    }

    // 文本搜索
    const q = searchQuery.value.trim()
    if (!q) return cmds
    const lq = q.toLowerCase()
    return cmds.filter(
      c =>
        c.name.toLowerCase().includes(lq) ||
        c.labelEn.toLowerCase().includes(lq) ||
        c.labelZh.toLowerCase().includes(lq) ||
        c.description.toLowerCase().includes(lq),
    )
  })

  // 分组计数（用于 tab badge）
  const typeCounts = computed(() => {
    const skills = allCommands.value.filter(c => c.type === 'skill').length
    const mcp = allCommands.value.filter(c => c.type === 'mcp').length
    return { skill: skills, mcp }
  })

  // Load skills from local Tauri backend
  async function loadSkills(): Promise<SlashCommand[]> {
    try {
      const list: any[] = await invoke('list_local_skills', { category: null })
      if (!Array.isArray(list)) return []
      return list
        .filter((s: any) => s.enabled !== false)
        .map((s: any) => ({
          name: s.name,
          labelEn: s.name,
          labelZh: s.description || s.category || 'Skill',
          description: s.description_en || '',
          type: 'skill' as const,
        }))
    } catch {
      return []
    }
  }

  // Load MCP servers
  async function loadMcp(): Promise<SlashCommand[]> {
    try {
      const list: any[] = await invoke('list_mcp_servers')
      if (!Array.isArray(list)) return []
      return list.map((s: any) => ({
        name: s.name || s.id,
        labelEn: s.name || s.id,
        labelZh: `${s.transport_type || 'stdio'} · ${s.tool_count || 0} tools`,
        description: s.command || s.url || '',
        type: 'mcp' as const,
      }))
    } catch {
      return []
    }
  }

  async function loadAll() {
    if (loaded.value) return
    loaded.value = true
    const [skills, mcp] = await Promise.all([loadSkills(), loadMcp()])
    allCommands.value = [...skills, ...mcp]
  }

  function reset() {
    showMenu.value = false
    slashPos.value = -1
    query.value = ''
    selectedIndex.value = -1
    buttonMode.value = false
  }

  /** 按钮触发：打开弹出面板显示所有 Skills + MCP */
  function openPopup() {
    buttonMode.value = true
    showMenu.value = true
    slashPos.value = 0
    searchQuery.value = ''
    activeTab.value = 'all'
    selectedIndex.value = -1
    loadAll()
  }

  /** 按钮触发：关闭弹出面板 */
  function closePopup() {
    reset()
  }

  function openAtPosition(pos: number, typeFilter?: string) {
    showMenu.value = true
    slashPos.value = pos
    selectedIndex.value = -1
    query.value = typeFilter ? `__type:${typeFilter}__` : ''
    loadAll() // lazy load
    textareaRef.value?.focus?.()
  }

  function selectCommand(cmd: SlashCommand) {
    // 按钮模式：通过回调返回选中项，不插入文本
    reset()
    onButtonSelect?.(cmd)
  }

  function handleInput(_newValue: string, _cursorPos: number) {
    // ★ / 命令已废弃，改为按钮弹出模式
    // 保留函数签名以兼容现有调用，不再检测 / 输入
    return
    /*
    // 按钮模式：不让文本输入干扰弹出面板
    if (buttonMode.value) return

    if (!newValue.trim()) {
      reset()
      return
    }

    const textBefore = newValue.slice(0, cursorPos)
    const match = textBefore.match(/(^|\s)\/(\S*)$/)

    if (!match) {
      reset()
      return
    }

    const pos = (match.index || 0) + match[1].length
    const q = match[2]

    // Special handling: "/skills" or "/skill" → 只显示 skills
    if (q === 'skills' || q === 'skill') {
      openAtPosition(pos, 'skill')
      return
    }
    if (q === 'mcp') {
      openAtPosition(pos, 'mcp')
      return
    }

    loadAll() // lazy load on first slash
    slashPos.value = pos
    query.value = q
    showMenu.value = true
    selectedIndex.value = -1
    */
  }

  function handleKeydown(e: KeyboardEvent): boolean {
    if (!showMenu.value) return false
    const cmds = filtered.value
    if (!cmds.length) {
      if (e.key === 'Escape') { e.preventDefault(); reset(); return true }
      return false
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      selectedIndex.value = selectedIndex.value < cmds.length - 1 ? selectedIndex.value + 1 : 0
      return true
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      selectedIndex.value = selectedIndex.value > 0 ? selectedIndex.value - 1 : cmds.length - 1
      return true
    }
    if (e.key === 'Tab' || e.key === 'Enter') {
      e.preventDefault()
      const idx = selectedIndex.value >= 0 ? selectedIndex.value : 0
      if (cmds[idx]) selectCommand(cmds[idx])
      return true
    }
    if (e.key === 'Escape') {
      e.preventDefault()
      reset()
      return true
    }
    return false
  }

  onUnmounted(() => { reset() })

  return {
    allCommands,
    filtered,
    showMenu,
    query,
    searchQuery,
    activeTab,
    typeCounts,
    selectedIndex,
    handleInput,
    handleKeydown,
    selectCommand,
    reset,
    openAtPosition,
    openPopup,
    closePopup,
    loadAll,
  }
}

// FastClaw · 「本次对话」的 Skill / MCP 白名单
//
// 与 mcpChat 的全局开关不同，这里是**按会话隔离**的：默认空（本次对话不启用任何
// Skill / MCP），用户在输入框的「工具」弹层里按需打开。只存在内存里，不落库。
import { ref } from 'vue'
import { defineStore } from 'pinia'

export interface ChatToolsSelection {
  /** 本次对话允许使用的 Skill 名称 */
  skills: string[]
  /** 本次对话允许使用的 MCP Server id */
  mcpServerIds: string[]
}

const EMPTY: ChatToolsSelection = { skills: [], mcpServerIds: [] }

export const useChatToolsStore = defineStore('chatTools', () => {
  const byConversation = ref<Record<string, ChatToolsSelection>>({})

  function selection(conversationId: string | null | undefined): ChatToolsSelection {
    if (!conversationId) return EMPTY
    return byConversation.value[conversationId] || EMPTY
  }

  function isSkillEnabled(conversationId: string | null | undefined, name: string): boolean {
    return selection(conversationId).skills.includes(name)
  }

  function isMcpServerEnabled(conversationId: string | null | undefined, id: string): boolean {
    return selection(conversationId).mcpServerIds.includes(id)
  }

  function toggleSkill(conversationId: string | null | undefined, name: string): void {
    if (!conversationId) return
    const cur = selection(conversationId)
    const next = cur.skills.includes(name)
      ? cur.skills.filter((n) => n !== name)
      : [...cur.skills, name]
    byConversation.value[conversationId] = { ...cur, skills: next }
  }

  function toggleMcpServer(conversationId: string | null | undefined, id: string): void {
    if (!conversationId) return
    const cur = selection(conversationId)
    const next = cur.mcpServerIds.includes(id)
      ? cur.mcpServerIds.filter((n) => n !== id)
      : [...cur.mcpServerIds, id]
    byConversation.value[conversationId] = { ...cur, mcpServerIds: next }
  }

  /** 会话关闭时清掉，避免 store 无界增长 */
  function forget(conversationId: string): void {
    delete byConversation.value[conversationId]
  }

  return {
    selection,
    isSkillEnabled,
    isMcpServerEnabled,
    toggleSkill,
    toggleMcpServer,
    forget,
  }
})

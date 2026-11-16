// FastClaw · 智能体管理 Store
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

/** 智能体偏好参数（采样配置） */
export interface AgentPreferences {
  temperature?: number | null
  top_p?: number | null
  max_tokens?: number | null
  presence_penalty?: number | null
  frequency_penalty?: number | null
}

export interface Agent {
  id: string
  agent_name: string
  agent_description: string | null
  agent_icon: string | null
  agent_prompt: string | null
  agent_type: string | null
  agent_model: string | null
  agent_knowledgebase: string | null
  agent_status: string
  agent_visibility: string
  agent_visibility_roles: string[] | null
  agent_tags: string[] | null
  agent_creator: string | null
  agent_cover: string | null
  agent_function_desc: string | null
  agent_input_params: any | null
  agent_output_results: any | null
  json_data: string | null
  created_time: string
  update_time: string
}

export interface AgentInput {
  agent_name: string
  agent_description?: string
  agent_icon?: string
  agent_prompt?: string
  agent_type?: string
  agent_model?: string
  agent_knowledgebase?: string
  agent_status?: string
  agent_visibility?: string
  agent_visibility_roles?: string[]
  agent_tags?: string[]
  agent_creator?: string
  agent_cover?: string
  agent_function_desc?: string
  agent_input_params?: any
  agent_output_results?: any
  json_data?: string
}

export interface ListAgentsParams {
  page?: number
  page_size?: number
  search?: string
  agent_type?: string
  agent_status?: string
  agent_visibility?: string
}

export interface PaginatedResult {
  items: Agent[]
  total: number
  page: number
  page_size: number
}

const items = ref<Agent[]>([])
const total = ref(0)
const loading = ref(false)

export function useAgentStore() {
  async function loadAgents(params?: ListAgentsParams): Promise<void> {
    loading.value = true
    try {
      const data = await invoke<PaginatedResult>('list_agents', {
        params: params || {},
      })
      items.value = data.items
      total.value = data.total
    } catch (e) {
      console.error('Failed to load agents:', e)
    } finally {
      loading.value = false
    }
  }

  async function getAgent(id: string): Promise<Agent | null> {
    return await invoke<Agent | null>('get_agent', { id })
  }

  async function createAgent(id: string, input: AgentInput) {
    await invoke('create_agent', { id, input })
    await loadAgents()
  }

  async function updateAgent(id: string, input: AgentInput) {
    await invoke('update_agent', { id, input })
    await loadAgents()
  }

  async function deleteAgent(id: string) {
    await invoke('delete_agent', { id })
    await loadAgents()
  }

  async function checkAgentName(name: string, excludeId?: string): Promise<boolean> {
    return await invoke<boolean>('check_agent_name', {
      params: { name, exclude_id: excludeId || null },
    })
  }

  async function getAgentIcon(iconPath: string): Promise<string | null> {
    if (!iconPath) return null
    try {
      return await invoke<string>('get_agent_icon', { iconPath })
    } catch (e) {
      console.error('Failed to get agent icon:', e)
      return null
    }
  }

  async function saveAgentIcon(agentId: string, file: File): Promise<string | null> {
    try {
      const reader = new FileReader()
      const dataUrl = await new Promise<string>((resolve) => {
        reader.onload = () => resolve(reader.result as string)
        reader.readAsDataURL(file)
      })
      const base64 = dataUrl.split(',')[1]
      const iconPath = await invoke<string>('save_agent_icon', {
        agentId,
        input: { icon_data: base64, file_name: file.name },
      })
      return iconPath
    } catch (e) {
      console.error('Failed to save agent icon:', e)
      return null
    }
  }

  return {
    items,
    total,
    loading,
    loadAgents,
    getAgent,
    createAgent,
    updateAgent,
    deleteAgent,
    checkAgentName,
    getAgentIcon,
    saveAgentIcon,
  }
}

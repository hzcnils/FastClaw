// FastClaw · 个人助理角色类型定义

export interface BoundMcp {
  mcpId: string
  mcpName: string
  status: string
  sortOrder: number
}

export interface BoundSkill {
  skillId: string
  skillName: string
  skillDesc: string
  sortOrder: number
}

export interface BoundKnowledgeBase {
  kbId: string
  kbName: string
  docCount: number
  sortOrder: number
}

export interface Role {
  id: string
  name: string
  description?: string
  avatar?: string
  systemPrompt: string
  defaultModel?: string
  temperature?: number
  contextTurns: number
  allowPublish: boolean
  isActive: boolean
  isPublished: boolean
  sourceType: 'user' | 'square'
  sourceId?: string
  squareCategory?: string
  squareDesc?: string
  publishedAt?: string
  notes?: string
  createdAt: string
  updatedAt: string
  mcps?: BoundMcp[]
  skills?: BoundSkill[]
  knowledgeBases?: BoundKnowledgeBase[]
}

export interface SquareRole {
  id: string
  originId?: string
  name: string
  description?: string
  avatar?: string
  category: string
  authorName?: string
  useCount: number
  starCount: number
  toolSummary: {
    mcps?: string[]
    skills?: string[]
  }
  publishedAt: string
}

export interface RoleStats {
  total: number
  published: number
  activeName: string
}

export interface RoleFilter {
  search: string
  sourceType: '' | 'user' | 'square'
  isActive: '' | 'true' | 'false'
}

export interface SquareFilter {
  search: string
  category: RoleCategory
  sortBy: 'newest' | 'mostUsed' | 'mostStarred'
  page: number
  pageSize: number
}

export type RoleCategory =
  | 'all'
  | 'work'
  | 'writing'
  | 'coding'
  | 'education'
  | 'life'
  | 'other'

export interface CreateRoleData {
  name: string
  description?: string
  avatar?: string
  systemPrompt: string
  defaultModel?: string
  temperature?: number
  contextTurns?: number
  allowPublish?: boolean
  notes?: string
  mcpIds?: string[]
  skillIds?: string[]
  knowledgeBaseIds?: string[]
}

export interface PagedResult<T> {
  items: T[]
  total: number
  page: number
  pageSize: number
}

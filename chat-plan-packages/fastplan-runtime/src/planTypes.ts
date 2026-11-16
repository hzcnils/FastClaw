export type FastPlanTaskPlanStatus = 'planning' | 'awaiting_confirmation' | 'executing' | 'paused' | 'completed' | 'failed' | 'cancelled'
export type FastPlanTaskPlanNodeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped'

export interface FastPlanTaskPlanNode {
  id: string
  title: string
  status: FastPlanTaskPlanNodeStatus
  sequence?: string
  depth?: number
  parentId?: string | null
  progress?: number
  startedAt?: string | number | Date
  completedAt?: string | number | Date
  error?: string
  output?: string
}

export interface FastPlanTaskPlanPhase {
  id: string
  title: string
  status: FastPlanTaskPlanNodeStatus
  nodes: FastPlanTaskPlanNode[]
}

export interface FastPlanTaskPlanPresentation {
  prompt?: string
  routeStages?: Array<{ id?: string; stage: string; label: string; detail?: string; status?: FastPlanTaskPlanNodeStatus; payload?: Record<string, unknown>; seq?: number; ts?: number }>
  thinkingSteps?: Array<{ id?: string; label: string; content: string; status?: FastPlanTaskPlanNodeStatus }>
  logs?: Array<{ id?: string; text: string; status?: FastPlanTaskPlanNodeStatus }>
  validation?: Array<{ id?: string; stage?: string; label: string; detail?: string; status?: FastPlanTaskPlanNodeStatus; error?: string }>
  inputSlots?: Array<{ id: string; label: string; required?: boolean; description?: string }>
  humanRequest?: { title?: string; question: string; options?: string[] }
  manualReviews?: Array<{ id?: string; title: string; detail?: string }>
  executionMode?: 'auto' | 'manual' | 'step' | string
}

export interface FastPlanTaskPlanExecution {
  id?: string
  title?: string
  status: FastPlanTaskPlanStatus
  summary?: string
  phases: FastPlanTaskPlanPhase[]
  presentation?: FastPlanTaskPlanPresentation
}

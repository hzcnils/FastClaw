import type { DefineComponent } from 'vue'

export interface FastPlanPlanningMessages {
  [key: string]: string
}

export const defaultPlanningMessages: FastPlanPlanningMessages
export const englishPlanningMessages: FastPlanPlanningMessages
export function resolvePlanningMessages(locale?: string, labels?: Partial<FastPlanPlanningMessages>): FastPlanPlanningMessages
export function formatPlanningMessage(messages: FastPlanPlanningMessages | undefined, key: string, variables?: Record<string, unknown>): string

export const FastPlanPlanningPanel: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanTaskList: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanEchartsBlock: DefineComponent<{ option: Record<string, unknown>; height?: string; source?: string; theme?: 'light' | 'dark' }, Record<string, unknown>, any>
export const FastPlanNodeDetailPanel: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanPlanningStatusCard: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanPlanPreview: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanFileGate: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanHumanCard: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanManualReviewPanel: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanExecutionDagPanel: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanNodeEditor: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanGraphView: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
export const FastPlanTaskBlock: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>

export function treeToDAG(tree: any, executionState?: Record<string, any>): { nodes: any[]; edges: any[]; groups: any[] }
export function getDAGStats(nodes: any[]): { total: number; completed: number; running: number; failed: number; pending: number }
export function deleteNodeWithDependencyFix(tree: any, nodeId: string): { success: boolean; removedFromPhaseId: string | null }
export function insertNodeAfter(tree: any, afterNodeId: string, newNode: any): { success: boolean }

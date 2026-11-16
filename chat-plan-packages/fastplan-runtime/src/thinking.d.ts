export interface FastPlanThinkingLog {
  type: string
  content: string
  toolName?: string
}
export function parseFastPlanThinking(content: unknown, phase?: unknown): FastPlanThinkingLog | null
export function pushFastPlanLog(logs: FastPlanThinkingLog[], entry: FastPlanThinkingLog | null | undefined): void

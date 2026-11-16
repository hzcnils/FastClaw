export interface FastPlanInputFile { name: string; path: string }
export interface FastPlanFileInjectionReport { matched: Array<{ slot: unknown; file: FastPlanInputFile }>; emptySlots: unknown[]; unmatched: FastPlanInputFile[] }
export function normalizeFastPlanPath(path: unknown): string | undefined
export function injectFastPlanFiles(tree: any, files?: FastPlanInputFile[]): FastPlanFileInjectionReport
export function buildFastPlanInputSlots(tree: any): any[]
export function ensureFastPlanInputSlots(tree: any): any

import { invoke } from '@tauri-apps/api/core'

export const FASTPLAN_RESUME_KEY = 'fastclaw.fastplan.resume.v1'
let pending: Promise<void> | undefined

/** 入口即上报，不等待根组件挂载或事件订阅；成功后本会话不重复覆盖令牌。 */
export function provideFastPlanResume(): Promise<void> {
  if (pending) return pending
  pending = (async () => {
    let data: unknown = null
    try { data = JSON.parse(localStorage.getItem(FASTPLAN_RESUME_KEY) || 'null') } catch { /* 存储不可用或损坏 */ }
    try { await invoke('provide_fastplan_resume_data', { data }) }
    catch { await invoke('provide_fastplan_resume_data', { data: null }) }
  })().catch(() => {
    // IPC 暂不可用时允许根组件稍后再试；不打印可能包含凭据的异常。
    pending = undefined
  })
  return pending
}

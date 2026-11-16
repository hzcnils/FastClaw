import { invoke } from '@tauri-apps/api/core'

export interface DependencyRecoveryStatus {
  id: string
  desired_state: 'running' | 'stopped'
  recovery_state: 'idle' | 'queued' | 'starting' | 'healthy' | 'backoff' | 'waiting_dependency' | 'waiting_shared_owner' | 'paused' | 'failed'
  generation: number
  recovery_attempt: number
  max_recovery_attempts: number
  retry_in_ms: number | null
  last_error: string | null
  enabled: boolean
  global_enabled: boolean
  service_enabled: boolean
  auto_start: boolean
  can_retry: boolean
}

export const getDependencyRecoveryStatus = () => invoke<DependencyRecoveryStatus[]>('get_dependency_recovery_status')

/** 启动期限由后端管理；前端只等待成功或确定的终态。 */
export async function waitForDependencyReady(id: string, signal?: AbortSignal): Promise<void> {
  for (;;) {
    if (signal?.aborted) throw new DOMException('已取消等待服务', 'AbortError')
    const statuses = await getDependencyRecoveryStatus()
    if (signal?.aborted) throw new DOMException('已取消等待服务', 'AbortError')
    const status = statuses.find(item => item.id === id)
    if (!status) throw new Error('未找到依赖服务状态')
    if (status.recovery_state === 'healthy') return
    if (status.desired_state === 'stopped' || ['failed', 'paused', 'waiting_shared_owner'].includes(status.recovery_state)) {
      throw new Error(status.last_error || '服务已停止或暂停恢复，请在依赖服务页面处理')
    }
    if (id === 'fastkg') {
      const blocked = statuses.find(item => ['qdrant', 'llama_server'].includes(item.id) &&
        (item.desired_state === 'stopped' || ['failed', 'paused', 'waiting_shared_owner'].includes(item.recovery_state)))
      if (blocked) throw new Error(blocked.last_error || '知识库依赖服务尚未就绪')
    }
    await new Promise(resolve => setTimeout(resolve, 500))
  }
}

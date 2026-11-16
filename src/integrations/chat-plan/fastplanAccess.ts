import { invoke } from '@tauri-apps/api/core'
import type { FastPlanAccess, FastPlanConnectionInfo, FastPlanRequestModel } from 'planflow-runtime'

let connectionInFlight: Promise<FastPlanConnectionInfo> | null = null

/** No token or model credentials are persisted. Concurrent callers share only the current IPC request. */
export const fastClawFastPlanAccess: FastPlanAccess = {
  getConnection: () => {
    if (connectionInFlight) return connectionInFlight
    connectionInFlight = invoke<FastPlanConnectionInfo>('get_fastplan_connection')
      .finally(() => { connectionInFlight = null })
    return connectionInFlight
  },
  async resolveModel(selection) {
    const id = selection && typeof selection === 'object' ? (selection as { id?: string }).id : undefined
    if (!id) throw new Error('请重新选择规划模型')
    return invoke<FastPlanRequestModel>('get_fastplan_model_config', { modelId: id })
  },
}

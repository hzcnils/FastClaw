import { invokeRaw } from '@/shared/tauri-ipc'
import {
  MODEL_MANAGEMENT_COMMANDS,
  type ModelImportItem,
  type SyncPreview,
  type SyncResult,
} from '../types/contracts'

export function getSyncPreview(): Promise<SyncPreview> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.syncPreview)
}

export function executeSync(): Promise<SyncResult> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.syncExecute)
}

export function importModelsJson(models: ModelImportItem[]): Promise<SyncResult> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.importModelsJson, { models })
}

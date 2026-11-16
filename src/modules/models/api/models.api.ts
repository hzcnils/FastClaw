import { invokeRaw } from '@/shared/tauri-ipc'
import {
  MODEL_MANAGEMENT_COMMANDS,
  type ModelUpdatePatch,
  type ModelExportItem,
  type RawModelExportResponse,
  type RawModelResponse,
  type RemoteModel,
} from '../types/contracts'

export function listModels(providerCode: string | null): Promise<RawModelResponse[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.listModels, { providerCode })
}

export function listModelsForExport(): Promise<RawModelExportResponse[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.listModelsForExport)
}

export function exportModels(includeCredentials: boolean): Promise<ModelExportItem[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.exportModels, { includeCredentials })
}

export function fetchRemoteModels(providerCode: string): Promise<RemoteModel[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.fetchRemoteModels, { providerCode })
}

export function saveRemoteModels(providerCode: string, models: RemoteModel[]): Promise<void> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.saveRemoteModels, { providerCode, models })
}

export function updateModels(ids: string[], updates: ModelUpdatePatch): Promise<void> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.updateModels, { ids, updates })
}

export function deleteModels(ids: string[]): Promise<void> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.deleteModels, { ids })
}

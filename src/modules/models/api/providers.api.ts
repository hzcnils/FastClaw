import { invokeRaw } from '@/shared/tauri-ipc'
import {
  MODEL_MANAGEMENT_COMMANDS,
  type AddProviderInput,
  type ProviderUpdatePatch,
  type ProviderRuntimeConfig,
  type ProviderSummary,
  type RawProviderResponse,
  type TestConnectionInput,
  type TestConnectionResult,
} from '../types/contracts'

export function listProviders(): Promise<RawProviderResponse[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.listProviders)
}

export function listProviderSummaries(): Promise<ProviderSummary[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.listProviderSummaries)
}

export function getProviderRuntimeConfig(providerCode: string): Promise<ProviderRuntimeConfig> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.getProviderRuntimeConfig, { providerCode })
}

export function addProvider(input: AddProviderInput): Promise<string> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.addProvider, { ...input })
}

export function updateProvider(id: string, updates: ProviderUpdatePatch): Promise<void> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.updateProvider, { id, updates })
}

export function deleteProvider(id: string): Promise<void> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.deleteProvider, { id })
}

export function testConnection(input: TestConnectionInput): Promise<TestConnectionResult> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.testConnection, { ...input })
}

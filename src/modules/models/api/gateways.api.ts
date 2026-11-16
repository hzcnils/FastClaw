import { invokeRaw } from '@/shared/tauri-ipc'
import {
  MODEL_MANAGEMENT_COMMANDS,
  type AddGatewayInput,
  type GatewayProvider,
  type GatewayWithProviders,
  type RawGatewayResponse,
} from '../types/contracts'

export function listGateways(): Promise<RawGatewayResponse[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.listGateways)
}

export function addGateway(input: AddGatewayInput): Promise<string> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.addGateway, { ...input })
}

export function listGatewayProviders(gatewayId: string): Promise<GatewayProvider[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.listGatewayProviders, { gatewayId })
}

export function listGatewaysWithProviders(): Promise<GatewayWithProviders[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.listGatewaysWithProviders)
}

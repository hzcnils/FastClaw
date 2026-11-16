import { invokeRaw } from '@/shared/tauri-ipc'
import {
  MODEL_MANAGEMENT_COMMANDS,
  type GetModelRatingsInput,
  type ModelRating,
  type RankedModel,
} from '../types/contracts'

export function getModelRatings(input: GetModelRatingsInput = {}): Promise<ModelRating[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.getModelRatings, {
    scope: input.scope ?? 'global',
    userId: input.userId ?? null,
  })
}

export function getModelRanking(): Promise<RankedModel[]> {
  return invokeRaw(MODEL_MANAGEMENT_COMMANDS.getModelRanking)
}

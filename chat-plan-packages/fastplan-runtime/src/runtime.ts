import { createFastPlanExecutionController, type FastPlanExecutionControllerOptions } from './executionController'

/** Backward-compatible runtime name. New integrations may use createFastPlanExecutionController explicitly. */
export function createFastPlanRuntime(options: FastPlanExecutionControllerOptions = {}) {
  return createFastPlanExecutionController(options)
}

export type FastPlanRuntimeOptions = FastPlanExecutionControllerOptions

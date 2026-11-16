import type { FastPlanExecutionController } from 'planflow-runtime'

/** UI intent only; session/turn identity is supplied by the owning host. */
export interface FastPlanInteractionCommand {
  type: 'confirm' | 'cancel-plan' | 'cancel-execution' | 'pause' | 'resume' | 'retry-node' | 'skip-node' | 'human-response' | 'edit-node-output' | 'edit-node-config'
  tree?: any
  attachments?: any[]
  nodeId?: string
  response?: any
  request?: any
  output?: any
  changes?: Record<string, any>
}

/** Standalone compatibility path. Managed hosts provide their own dispatcher. */
export function executeFastPlanInteraction(runtime: FastPlanExecutionController, command: FastPlanInteractionCommand) {
  switch (command.type) {
    case 'confirm': return runtime.confirmPlan(command.tree, command.attachments)
    case 'cancel-plan': return runtime.cancelPlan()
    case 'cancel-execution': return runtime.cancelExecution()
    case 'pause': return runtime.pauseExecution()
    case 'resume': return runtime.resumeExecution()
    case 'retry-node': return command.nodeId ? runtime.retryNode(command.nodeId) : false
    case 'skip-node': return command.nodeId ? runtime.skipNode(command.nodeId) : false
    case 'human-response': return runtime.respondHumanCheckpoint(command.response, command.request)
    case 'edit-node-output': return command.nodeId ? runtime.updateNodeOutput(command.nodeId, command.output) : false
    case 'edit-node-config': return command.nodeId ? runtime.updateNextNodeConfig(command.nodeId, command.changes) : false
  }
}

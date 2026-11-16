export interface DAGNode {
  id: string
  name: string
  status: string
  level: number
  nodeType: string
  phaseId: string
}

export interface DAGEdge {
  from: string
  to: string
  type: 'sequential' | 'dependency'
}

export interface DAGResult {
  nodes: DAGNode[]
  edges: DAGEdge[]
}

export interface TaskTreeNode {
  id: string
  name: string
  node_type?: { type: string; config?: Record<string, unknown> }
  children?: TaskTreeNode[]
  dependencies?: string[]
}

export interface TaskTree {
  root: TaskTreeNode
  tree_id?: string
}

export function treeToDAG(tree: TaskTree, executionState: Record<string, { status?: string }> = {}): DAGResult {
  if (!tree?.root?.children) return { nodes: [], edges: [] }

  const nodes: DAGNode[] = []
  const edges: DAGEdge[] = []
  const phases = tree.root.children

  let prevPhaseLastTaskId: string | null = null

  phases.forEach((phase, pIdx) => {
    nodes.push({
      id: phase.id,
      name: phase.name,
      level: pIdx * 2,
      status: executionState[phase.id]?.status || 'pending',
      nodeType: 'Phase',
      phaseId: phase.id,
    })

    if (prevPhaseLastTaskId) {
      edges.push({ from: prevPhaseLastTaskId, to: phase.id, type: 'sequential' })
    }

    const tasks = phase.children || []

    if (tasks.length > 0) {
      edges.push({ from: phase.id, to: tasks[0].id, type: 'sequential' })
    }

    tasks.forEach((task, tIdx) => {
      const nodeType = (task.node_type as any)?.type || 'LlmTask'

      nodes.push({
        id: task.id,
        name: task.name,
        level: pIdx * 2 + 1,
        status: executionState[task.id]?.status || 'pending',
        nodeType,
        phaseId: phase.id,
      })

      if (task.dependencies && task.dependencies.length > 0) {
        task.dependencies.forEach(depId => {
          edges.push({ from: depId, to: task.id, type: 'dependency' })
        })
      } else if (tIdx > 0) {
        edges.push({ from: tasks[tIdx - 1].id, to: task.id, type: 'sequential' })
      }
    })

    prevPhaseLastTaskId = tasks.length > 0 ? tasks[tasks.length - 1].id : phase.id
  })

  return { nodes, edges }
}

export function getDAGStats(nodes: DAGNode[]) {
  const total = nodes.filter(n => n.nodeType !== 'Phase').length
  const completed = nodes.filter(n => n.nodeType !== 'Phase' && n.status === 'completed').length
  const running = nodes.filter(n => n.nodeType !== 'Phase' && n.status === 'running').length
  const failed = nodes.filter(n => n.nodeType !== 'Phase' && n.status === 'failed').length
  const pending = total - completed - running - failed
  return { total, completed, running, failed, pending }
}

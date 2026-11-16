import { resolveNodeModel } from './nodeModel'

/**
 * treeToDAG.js - 将 TaskTree 转换为 GraphView 所需的 nodes + edges + groups 格式
 *
 * 输入: tree.root.children = [Phase{children:[Task1, Task2]}, ...]
 * 输出: { nodes, edges, groups }
 *   - nodes: 仅任务节点（Phase 不再作为独立节点）
 *   - groups: 阶段区域信息 [{ id, name, taskIds[], status, phaseIdx }]
 */

/**
 * 聚合阶段状态（基于子任务状态推导）
 */
function aggregatePhaseStatus(tasks, executionState) {
  const statuses = tasks.map(t => executionState[t.id]?.status || 'pending')
  if (statuses.some(s => s === 'running'))   return 'running'
  if (statuses.some(s => s === 'failed'))    return 'failed'
  if (statuses.every(s => s === 'completed')) return 'completed'
  return 'pending'
}

/**
 * @param {Object} tree - 后端返回的 TaskTree
 * @param {Object} executionState - 当前执行状态 map
 * @returns {{ nodes: Array, edges: Array, groups: Array }}
 */
export function treeToDAG(tree, executionState = {}) {
  if (!tree?.root?.children) return { nodes: [], edges: [], groups: [] }

  const nodes = []
  const edges = []
  const groups = []
  const phases = tree.root.children

  let prevPhaseLastTaskId = null

  phases.forEach((phase, pIdx) => {
    const tasks = phase.children || []

    // 阶段区域（组）信息 — 不作为节点，仅用于绘制区域框
    groups.push({
      id: phase.id,
      name: phase.name,
      taskIds: tasks.map(t => t.id),
      status: executionState[phase.id]?.status || aggregatePhaseStatus(tasks, executionState),
      phaseIdx: pIdx
    })

    tasks.forEach((task, tIdx) => {
      const nodeType = task.node_type?.type || 'LlmTask'
      const model = resolveNodeModel(task, executionState[task.id])

      nodes.push({
        id: task.id,
        name: task.name,
        level: pIdx,               // 每个阶段一行，不再交替 Phase/Task 行
        status: executionState[task.id]?.status || 'pending',
        nodeType,
        model,
        phaseId: phase.id,
        hasSubTasks: nodeType === 'AgentTask' && (task.node_type?.config?.sub_tasks?.length > 0),
        assignedTools: task.node_type?.config?.assigned_tools || []
      })

      if (task.dependencies && task.dependencies.length > 0) {
        // 显式依赖边
        task.dependencies.forEach(depId => {
          edges.push({ from: depId, to: task.id, type: 'dependency' })
        })
      } else if (tIdx > 0) {
        // 同阶段内顺序边
        edges.push({ from: tasks[tIdx - 1].id, to: task.id, type: 'sequential' })
      } else if (prevPhaseLastTaskId) {
        // 跨阶段顺序边：上一阶段末任务 → 本阶段首任务
        edges.push({ from: prevPhaseLastTaskId, to: task.id, type: 'sequential' })
      }
    })

    if (tasks.length > 0) {
      prevPhaseLastTaskId = tasks[tasks.length - 1].id
    }
  })

  // Inject Start/End virtual nodes
  if (nodes.length > 0) {
    const targetIds = new Set(edges.map(e => e.to))
    const sourceIds = new Set(edges.map(e => e.from))
    const startNodes = nodes.filter(n => !targetIds.has(n.id))
    const endNodes = nodes.filter(n => !sourceIds.has(n.id))

    const minLevel = Math.min(...nodes.map(n => n.level))
    const maxLevel = Math.max(...nodes.map(n => n.level))

    nodes.unshift({
      id: '__start__',
      name: 'Start',
      level: minLevel - 1,
      status: 'completed',
      nodeType: 'special',
      phaseId: null,
      hasSubTasks: false
    })
    nodes.push({
      id: '__end__',
      name: 'End',
      level: maxLevel + 1,
      status: endNodes.every(n => n.status === 'completed') ? 'completed' : 'pending',
      nodeType: 'special',
      phaseId: null,
      hasSubTasks: false
    })

    startNodes.forEach(n => {
      edges.push({ from: '__start__', to: n.id, type: 'virtual' })
    })
    endNodes.forEach(n => {
      edges.push({ from: n.id, to: '__end__', type: 'virtual' })
    })
  }

  return { nodes, edges, groups }
}

/**
 * 获取 DAG 统计信息（仅统计任务节点，Phase 已不在 nodes 中）
 */
export function getDAGStats(nodes) {
  const total = nodes.length
  const completed = nodes.filter(n => n.status === 'completed').length
  const running   = nodes.filter(n => n.status === 'running').length
  const failed    = nodes.filter(n => n.status === 'failed').length
  const pending   = total - completed - running - failed
  return { total, completed, running, failed, pending }
}

/**
 * 删除节点并自动修复下游依赖关系（PRD §1.2）
 *
 * 规则：被删节点的下游依赖节点，自动重指向被删节点的上游节点。
 *
 * @param {Object} tree - 可变的 TaskTree 对象（直接修改）
 * @param {String} nodeId - 要删除的节点 ID
 * @returns {{ success: boolean, removedFromPhaseId: string | null }}
 */
export function deleteNodeWithDependencyFix(tree, nodeId) {
  if (!tree?.root?.children) return { success: false, removedFromPhaseId: null }

  let targetNode = null
  let targetPhase = null
  let targetIndexInPhase = -1
  let isPhase = false

  for (const phase of tree.root.children) {
    if (phase.id === nodeId) {
      targetNode = phase
      isPhase = true
      break
    }
    const idx = (phase.children || []).findIndex(t => t.id === nodeId)
    if (idx !== -1) {
      targetNode = phase.children[idx]
      targetPhase = phase
      targetIndexInPhase = idx
      break
    }
  }

  if (!targetNode) return { success: false, removedFromPhaseId: null }

  // 确定被删节点的上游（用于重定向下游依赖）
  let upstreamIds = []
  if (targetNode.dependencies && targetNode.dependencies.length > 0) {
    upstreamIds = [...targetNode.dependencies]
  } else if (!isPhase && targetPhase && targetIndexInPhase > 0) {
    upstreamIds = [targetPhase.children[targetIndexInPhase - 1].id]
  }

  // 修复下游节点的显式依赖
  for (const phase of tree.root.children) {
    if (phase.id === nodeId) continue
    for (const task of (phase.children || [])) {
      if (Array.isArray(task.dependencies) && task.dependencies.includes(nodeId)) {
        task.dependencies = [
          ...new Set([
            ...task.dependencies.filter(d => d !== nodeId),
            ...upstreamIds
          ])
        ]
      }
    }
  }

  // 实际删除
  if (isPhase) {
    const idx = tree.root.children.findIndex(p => p.id === nodeId)
    if (idx !== -1) tree.root.children.splice(idx, 1)
    return { success: true, removedFromPhaseId: null }
  } else {
    targetPhase.children.splice(targetIndexInPhase, 1)
    return { success: true, removedFromPhaseId: targetPhase.id }
  }
}

/**
 * 在指定节点之后插入新节点，并自动更新依赖关系（PRD §1.2）
 *
 * @param {Object} tree - 可变的 TaskTree 对象
 * @param {String} afterNodeId - 在此节点之后插入（Phase 节点则插入为第一个子任务）
 * @param {Object} newNode - 新节点对象
 * @returns {{ success: boolean }}
 */
export function insertNodeAfter(tree, afterNodeId, newNode) {
  if (!tree?.root?.children || !afterNodeId || !newNode) return { success: false }

  for (const phase of tree.root.children) {
    if (phase.id === afterNodeId) {
      if (!phase.children) phase.children = []
      newNode.dependencies = newNode.dependencies || []
      phase.children.unshift(newNode)
      return { success: true }
    }

    const idx = (phase.children || []).findIndex(t => t.id === afterNodeId)
    if (idx !== -1) {
      const afterNode = phase.children[idx]
      if (!newNode.dependencies || newNode.dependencies.length === 0) {
        newNode.dependencies = [afterNode.id]
      }
      // 让原来的下一个节点改为依赖新节点
      const nextNode = phase.children[idx + 1]
      if (nextNode) {
        if (!nextNode.dependencies || nextNode.dependencies.length === 0) {
          nextNode.dependencies = [newNode.id]
        } else if (nextNode.dependencies.includes(afterNode.id)) {
          nextNode.dependencies = nextNode.dependencies.map(d => d === afterNode.id ? newNode.id : d)
        }
      }
      phase.children.splice(idx + 1, 0, newNode)
      return { success: true }
    }
  }

  return { success: false }
}

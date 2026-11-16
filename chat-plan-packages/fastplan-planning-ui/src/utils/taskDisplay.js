export function flattenTaskNodes(nodes = [], prefix = '', parentId = null, depth = 0) {
  const rows = []
  nodes.forEach((node, index) => {
    const sequence = prefix ? `${prefix}.${index + 1}` : String(index + 1)
    rows.push({ node, sequence, parentId, depth })
    rows.push(...flattenTaskNodes(node?.children || [], sequence, node?.id || null, depth + 1))
  })
  return rows
}

export function planPhaseRows(tree) {
  return (tree?.root?.children || []).map((phase, phaseIndex) => {
    const tasks = flattenTaskNodes(phase?.children || [], String(phaseIndex + 1), phase?.id || null, 0)
    return { phase, phaseIndex, tasks, total: tasks.length }
  })
}

export function lastPlanTaskId(tree) {
  const tasks = planPhaseRows(tree).flatMap(row => row.tasks)
  return tasks[tasks.length - 1]?.node?.id || null
}

export function revealedPhaseRows(tree, executionState = {}, revealAll = false) {
  return planPhaseRows(tree)
    .map(row => ({
      ...row,
      tasks: revealAll ? row.tasks : row.tasks.filter(({ node }) =>
        (executionState?.[node?.id]?.status || 'pending') !== 'pending'),
    }))
    .filter(row => revealAll || row.tasks.length > 0)
}

export function taskProgress(tree, executionState = {}) {
  const rows = planPhaseRows(tree).flatMap(row => row.tasks)
  let completed = 0, skipped = 0, failed = 0, running = 0
  for (const { node } of rows) {
    const status = executionState?.[node?.id]?.status || 'pending'
    if (status === 'completed') completed += 1
    else if (status === 'skipped') skipped += 1
    else if (status === 'failed') failed += 1
    else if (status === 'running') running += 1
  }
  return { total: rows.length, completed, skipped, failed, running, success: completed + skipped }
}

export function groupConfirmedHistoryByNode(tree, confirmedHistory = []) {
  const nodes = planPhaseRows(tree).flatMap(row => row.tasks).map(row => row.node)
  const nodeIds = new Set(nodes.map(node => String(node?.id || '')).filter(Boolean))
  const nodeIdsByName = new Map()
  for (const node of nodes) {
    const name = String(node?.name || '')
    if (!name) continue
    nodeIdsByName.set(name, [...(nodeIdsByName.get(name) || []), String(node.id)])
  }

  const legacyNameOffsets = new Map()
  const byNodeId = {}
  const unmatched = []
  for (const item of confirmedHistory || []) {
    let nodeId = String(item?.nodeId || item?.node_id || '')
    if (!nodeIds.has(nodeId)) {
      const name = String(item?.completedNodeName || '')
      const candidates = nodeIdsByName.get(name) || []
      const offset = legacyNameOffsets.get(name) || 0
      nodeId = candidates[offset] || ''
      if (nodeId) legacyNameOffsets.set(name, offset + 1)
    }
    if (!nodeId || !nodeIds.has(nodeId)) {
      unmatched.push(item)
      continue
    }
    if (!byNodeId[nodeId]) byNodeId[nodeId] = []
    byNodeId[nodeId].push(item)
  }
  return { byNodeId, unmatched }
}

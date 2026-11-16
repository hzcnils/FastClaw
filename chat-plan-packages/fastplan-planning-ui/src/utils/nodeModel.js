function text(value) {
  if (value === null || value === undefined) return ''
  return typeof value === 'string' || typeof value === 'number' ? String(value).trim() : ''
}

/** Normalize the model override carried by a planning node for ChatModelIcon. */
export function resolveNodeModel(node, executionState = null) {
  const config = node?.node_type?.config || node?.config || {}
  const raw = config.model ?? config.model_id ?? config.modelId
    ?? node?.model ?? node?.model_id ?? node?.modelId
    ?? executionState?.model ?? executionState?.model_id ?? executionState?.modelId

  if (raw && typeof raw === 'object') {
    const id = text(raw.id ?? raw.model ?? raw.model_id ?? raw.modelId)
    const name = text(raw.name ?? raw.model_name ?? raw.modelName) || id
    return { id, name }
  }

  const id = text(raw)
  const name = text(config.model_name ?? config.modelName
    ?? node?.model_name ?? node?.modelName
    ?? executionState?.model_name ?? executionState?.modelName) || id
  return { id, name }
}

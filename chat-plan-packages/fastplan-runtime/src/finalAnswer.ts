/** Normalize the terminal execution payload produced by FastPlan across API versions. */
export function normalizeFastPlanFinalAnswer(message: any) {
  const finalOutput = message?.final_output || {}
  const rawContent = message?.content ?? finalOutput.content ?? finalOutput.summary ?? finalOutput.answer ?? finalOutput.result ?? ''
  const content = typeof rawContent === 'string'
    ? rawContent
    : rawContent == null
      ? ''
      : JSON.stringify(rawContent, null, 2)
  const finalSources = Array.isArray(message?.final_sources)
    ? message.final_sources
    : Array.isArray(finalOutput.source_nodes)
      ? finalOutput.source_nodes
      : []
  const inputRefs = Array.isArray(message?.input_refs)
    ? message.input_refs
    : Array.isArray(finalOutput.input_refs)
      ? finalOutput.input_refs
      : []

  return {
    treeId: message?.tree_id || null,
    content,
    finalNodeId: message?.final_node_id || finalOutput.node_id || null,
    finalNodeName: message?.final_node_name || finalOutput.node_name || '',
    finalNodeType: message?.final_node_type || finalOutput.node_type || '',
    finalOutputKey: message?.final_output_key || finalOutput.output_key || '',
    finalSources,
    inputRefs,
    finalOutput,
  }
}

/** Keep artifact URLs in structured file data instead of exposing backend boilerplate in the answer. */
export function formatFastPlanFinalContent(content: unknown, files: Array<{ name?: string; url?: string }> = []) {
  const text = typeof content === 'string' ? content.trim() : ''
  if (!text || files.length === 0 || !isArtifactCompletionBoilerplate(text)) return text
  if (files.length > 1) return `已生成 ${files.length} 个文件，可在下方下载。`
  return isDocumentFile(files[0]?.name || files[0]?.url || '')
    ? '文档已生成，可在下方下载。'
    : '文件已生成，可在下方下载。'
}

function isArtifactCompletionBoilerplate(text: string) {
  return /^task\s+completed\.\s*\d+\s+file\(s\)\s+generated\.\s*download:\s*\S+\s*\.?$/i.test(text)
    || /^task\s+completed\.\s*download:\s*\S+\s*\.?$/i.test(text)
}

function isDocumentFile(value: string) {
  return /\.(?:docx?|pdf|odt|rtf|txt|md)(?:\?|$)/i.test(value)
}

// FastPlan plan input-slot inference and file injection. This is deliberately
// framework-free so every host uses the same template and execution behavior.
export function normalizeFastPlanPath(path) {
  if (!path) return path
  return String(path).replace(/\\/g, '/').replace(/^\/\/\?\//, '').replace(/^\/\?\//, '')
}

const HINTS = [
  { keys: ['word_path', 'word', 'doc_path', 'docx_path', 'word_file', 'doc_file', 'docfile', 'wordfile', 'input_word', 'word_doc'], exts: ['docx', 'doc'] },
  { keys: ['excel_path', 'excel', 'xlsx_path', 'sheet_path', 'excel_file', 'xls_file', 'xlsx_file', 'input_excel', 'excelfile', 'sheet'], exts: ['xlsx', 'xls'] },
  { keys: ['pdf_path', 'pdf', 'pdf_file', 'pdffile'], exts: ['pdf'] },
]
const extension = name => (/\.([a-z0-9]+)$/i.exec(name || '')?.[1] || '').toLowerCase()
const accepted = key => HINTS.find(hint => hint.keys.some(item => String(key).toLowerCase().includes(item)))?.exts || null

function collectSlots(tree) {
  if (Array.isArray(tree?.metadata?.input_slots) && tree.metadata.input_slots.length) return tree.metadata.input_slots.map(slot => ({ nodeId: slot.node_id, argKey: slot.arg_key, exts: (slot.accept || []).map(item => item.toLowerCase()), label: slot.label || slot.arg_key }))
  const slots = []
  const walk = node => {
    const args = node?.node_type?.config?.arguments
    if (args && typeof args === 'object') for (const key of Object.keys(args)) { const exts = accepted(key); if (exts) slots.push({ nodeId: node.id, argKey: key, exts, label: key }) }
    for (const child of node?.children || []) walk(child)
  }
  walk(tree?.root)
  return slots
}

export function injectFastPlanFiles(tree, files = []) {
  const report = { matched: [], emptySlots: [], unmatched: [] }
  const used = new Set()
  const setArgument = (nodeId, key, value) => {
    const walk = node => {
      if (!node) return false
      if (node.id === nodeId) { node.node_type ||= {}; node.node_type.config ||= {}; node.node_type.config.arguments ||= {}; node.node_type.config.arguments[key] = value; node.metadata ||= {}; node.metadata[key] = value; return true }
      return (node.children || []).some(walk)
    }
    walk(tree?.root)
  }
  for (const slot of collectSlots(tree)) {
    const file = files.find(item => !used.has(item.path) && slot.exts.includes(extension(item.name)))
    if (!file) { report.emptySlots.push(slot); continue }
    setArgument(slot.nodeId, slot.argKey, normalizeFastPlanPath(file.path)); used.add(file.path); report.matched.push({ slot, file })
  }
  report.unmatched = files.filter(file => !used.has(file.path))
  return report
}

export function buildFastPlanInputSlots(tree) {
  if (!tree?.root) return []
  const explicit = { word: { argKey: 'word_path', accept: ['docx', 'doc'], label: '上传 Word 文档' }, excel: { argKey: 'excel_path', accept: ['xlsx', 'xls'], label: '上传 Excel 文件' }, pdf: { argKey: 'pdf_path', accept: ['pdf'], label: '上传 PDF 文件' } }
  const slots = []
  const walk = node => {
    if (!node) return
    const input = node.metadata?.file_input
    if (input && input !== 'none' && explicit[input]) { const spec = explicit[input]; slots.push({ node_id: node.id, arg_key: spec.argKey, accept: spec.accept, label: node.name || node.title || spec.label }) }
    else for (const key of Object.keys(node.node_type?.config?.arguments || {})) { const exts = accepted(key); if (exts) slots.push({ node_id: node.id, arg_key: key, accept: exts, label: node.name || node.title || `上传文件(${exts.join('/')})` }) }
    for (const child of node.children || []) walk(child)
  }
  walk(tree.root)
  return slots
}

export function ensureFastPlanInputSlots(tree) {
  if (!tree || tree.metadata?.input_slots?.length) return tree
  const slots = buildFastPlanInputSlots(tree)
  if (slots.length) { tree.metadata ||= {}; tree.metadata.input_slots = slots }
  return tree
}

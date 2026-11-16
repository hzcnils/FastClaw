/**
 * 图谱节点配色（实体标签用）。
 * 自 FastKG 迁移，仅供分块查看页实体标签着色。
 */
export const KG_NODE_COLORS: Record<string, string> = {
  Person: '#1677ff',
  Organization: '#722ed1',
  Location: '#13c2c2',
  Event: '#fa8c16',
  Concept: '#eb2f96',
  Document: '#2f54eb',
  Default: '#8c8c8c',
}

export const KG_COLOR_PALETTE = ['#1677ff', '#13c2c2', '#52c41a', '#fa8c16', '#f5222d', '#eb2f96', '#722ed1']

const dynamicColorMap: Record<string, string> = {}

export function getKGNodeColor(label: string): string {
  if (KG_NODE_COLORS[label]) return KG_NODE_COLORS[label]
  if (!dynamicColorMap[label]) {
    const idx = Object.keys(dynamicColorMap).length % KG_COLOR_PALETTE.length
    dynamicColorMap[label] = KG_COLOR_PALETTE[idx]
  }
  return dynamicColorMap[label]
}

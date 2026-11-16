// FastClaw · 工具名归一化（jmis_search 折叠渲染专用）
// 后端 WebSearchTool 的规范名是 jmis_search，历史 / 别名路径可能有
// web_search / WebSearch / websearch 等变体，这里统一识别。

const WEB_SEARCH_TOOLS = new Set([
  'jmis_search',
  'jmissearch',
  'web_search',
  'websearch',
  'WebSearch',
])

/** 判断某工具调用是否为「联网搜索」，命中则走折叠列表 + 右侧预览渲染。 */
export function isWebSearchTool(name: string): boolean {
  if (!name) return false
  return WEB_SEARCH_TOOLS.has(name) || WEB_SEARCH_TOOLS.has(name.toLowerCase())
}

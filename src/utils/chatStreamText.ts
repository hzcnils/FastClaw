// 与后端 split_think_blocks 保持一致；流式时暂存尚未完整到达的标签。
function withoutPartialTag(text: string, tags: string[]): string {
  const lower = text.toLowerCase()
  for (let length = Math.min(text.length, Math.max(...tags.map(tag => tag.length)) - 1); length > 0; length--) {
    if (tags.some(tag => tag.startsWith(lower.slice(-length)))) return text.slice(0, -length)
  }
  return text
}

export function parseChatStreamText(raw: string) {
  let content = ''
  const thoughts: string[] = []
  const opener = /<(think|thinking)>/ig
  let cursor = 0
  while (cursor < raw.length) {
    opener.lastIndex = cursor
    const match = opener.exec(raw)
    if (!match) {
      content += withoutPartialTag(raw.slice(cursor), ['<think>', '<thinking>'])
      break
    }
    content += raw.slice(cursor, match.index)
    const start = opener.lastIndex
    const closer = `</${match[1].toLowerCase()}>`
    const end = raw.toLowerCase().indexOf(closer, start)
    if (end < 0) {
      thoughts.push(withoutPartialTag(raw.slice(start), [closer]))
      return { content, thinkContent: thoughts.filter(Boolean).join('\n\n'), inThink: true }
    }
    thoughts.push(raw.slice(start, end))
    cursor = end + closer.length
  }
  return { content, thinkContent: thoughts.filter(Boolean).join('\n\n'), inThink: false }
}

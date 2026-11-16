/** Wrap markdown `<table>` so mobile hosts can keep table layout and scroll horizontally. */
export function wrapMarkdownTables(html: string): string {
  if (!html || !html.includes('<table')) return html
  return html.replace(/<table\b[\s\S]*?<\/table>/gi, (tableHtml, offset, source) => {
    const before = source.slice(Math.max(0, offset - 80), offset)
    if (/jcr-md-table-scroll["']?\s*>\s*$/.test(before) || /class=["'][^"']*jcr-md-table-scroll/.test(before)) {
      return tableHtml
    }
    return `<div class="jcr-md-table-scroll">${tableHtml}</div>`
  })
}

import type { ChatReadingAnchor } from './tabState'

export function captureReadingPosition(wrap: HTMLElement): { top: number; anchor: ChatReadingAnchor | null } {
  const top = wrap.scrollTop
  const viewport = wrap.getBoundingClientRect()
  for (const node of Array.from(wrap.querySelectorAll<HTMLElement>('[data-message-id], [data-entry-id]'))) {
    const rect = node.getBoundingClientRect()
    if (rect.bottom <= viewport.top || rect.top >= viewport.bottom) continue
    const attribute = node.hasAttribute('data-message-id') ? 'data-message-id' : 'data-entry-id'
    return { top, anchor: { attribute, id: node.getAttribute(attribute)!, offset: rect.top - viewport.top } }
  }
  return { top, anchor: null }
}

export function restoreReadingPosition(wrap: HTMLElement, top: number, anchor: ChatReadingAnchor | null) {
  // No interpolated selector: arbitrary message ids cannot escape a CSS selector.
  const node = anchor && Array.from(wrap.querySelectorAll<HTMLElement>(`[${anchor.attribute}]`))
    .find(item => item.getAttribute(anchor.attribute) === anchor.id)
  wrap.scrollTop = node && anchor
    ? wrap.scrollTop + node.getBoundingClientRect().top - wrap.getBoundingClientRect().top - anchor.offset
    : top
}

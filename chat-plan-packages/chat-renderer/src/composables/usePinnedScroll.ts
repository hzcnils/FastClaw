import { nextTick, onBeforeUnmount, onMounted, ref, watch, type Ref } from 'vue'

export interface PinnedScrollOptions {
  root: Ref<HTMLElement | null>
  firstKey: () => unknown
  lastKey: () => unknown
  contentKey: () => unknown
  autoScroll: () => boolean
  canLoadOlder?: () => boolean
  onLoadOlder?: () => void
  onReady?: (element: HTMLElement) => void
  onScroll?: (element: HTMLElement) => void
  bottomThreshold?: number
  topThreshold?: number
}

export function usePinnedScroll(options: PinnedScrollOptions) {
  const userScrolledUp = ref(false)
  const hasNewMessages = ref(false)
  const bottomThreshold = options.bottomThreshold ?? 16
  const topThreshold = options.topThreshold ?? 80

  function updateScrollState() {
    const node = options.root.value
    if (!node) return
    const distance = Math.max(0, node.scrollHeight - node.scrollTop - node.clientHeight)
    userScrolledUp.value = node.scrollHeight > node.clientHeight && distance > bottomThreshold
    if (!userScrolledUp.value) hasNewMessages.value = false
  }

  function handleScroll() {
    const node = options.root.value
    if (!node) return
    updateScrollState()
    if (node.scrollTop < topThreshold && options.canLoadOlder?.()) options.onLoadOlder?.()
    options.onScroll?.(node)
  }

  function scrollToBottom(behavior: ScrollBehavior = 'smooth') {
    const node = options.root.value
    if (!node) return
    if (typeof node.scrollTo === 'function') node.scrollTo({ top: node.scrollHeight, behavior })
    else node.scrollTop = node.scrollHeight
    userScrolledUp.value = false
    hasNewMessages.value = false
  }

  let resizeObserver: ResizeObserver | undefined

  let previousHeight = 0
  watch(options.firstKey, async (next, previous) => {
    const node = options.root.value
    if (node && previous && next !== previous) previousHeight = node.scrollHeight
    await nextTick()
    if (node && previousHeight) {
      node.scrollTop += node.scrollHeight - previousHeight
      previousHeight = 0
    }
  })

  watch(options.lastKey, async (next, previous) => {
    if (!next || next === previous) return
    if (userScrolledUp.value) {
      hasNewMessages.value = true
      return
    }
    if (options.autoScroll()) {
      await nextTick()
      scrollToBottom('smooth')
    }
  })

  watch(options.contentKey, async () => {
    await nextTick()
    if (options.autoScroll() && !userScrolledUp.value) scrollToBottom('auto')
    else updateScrollState()
  }, { deep: true })

  onMounted(async () => {
    const node = options.root.value
    if (!node) return
    options.onReady?.(node)
    await nextTick()
    updateScrollState()
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(updateScrollState)
      resizeObserver.observe(node)
    }
  })

  onBeforeUnmount(() => resizeObserver?.disconnect())

  return { userScrolledUp, hasNewMessages, handleScroll, scrollToBottom, updateScrollState }
}

import { onBeforeUnmount, ref, watch, type Ref } from 'vue'

export type FastPlanDensity = 'auto' | 'comfortable' | 'compact'
export type ResolvedFastPlanDensity = Exclude<FastPlanDensity, 'auto'>

const DENSITY_HYSTERESIS_PX = 20

export function useResponsiveDensity(
  target: Readonly<Ref<HTMLElement | null>>,
  mode: () => FastPlanDensity,
  breakpoint: () => number,
) {
  const density = ref<ResolvedFastPlanDensity>(mode() === 'compact' ? 'compact' : 'comfortable')
  let observer: ResizeObserver | null = null

  function resolveWidth(width: number) {
    const limit = Math.max(0, Number(breakpoint()) || 700)
    if (density.value === 'compact') {
      if (width >= limit + DENSITY_HYSTERESIS_PX) density.value = 'comfortable'
      return
    }
    if (width <= limit) density.value = 'compact'
  }

  function bind() {
    observer?.disconnect()
    observer = null

    const requested = mode()
    if (requested !== 'auto') {
      density.value = requested
      return
    }

    const element = target.value
    if (!element) return
    const initialWidth = element.getBoundingClientRect().width || element.clientWidth
    if (initialWidth > 0) resolveWidth(initialWidth)
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(entries => {
        const width = entries[0]?.contentRect.width
        if (Number.isFinite(width) && Number(width) > 0) resolveWidth(Number(width))
      })
      observer.observe(element)
    }
  }

  watch([target, mode, breakpoint], bind, { immediate: true, flush: 'post' })
  onBeforeUnmount(() => observer?.disconnect())

  return density
}

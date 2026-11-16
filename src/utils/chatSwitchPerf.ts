export const CHAT_SWITCH_PERF_DEBUG_KEY = 'fastclaw.chatSwitchPerf.debug'

const MAX_SAMPLES = 100
const LATE_METRIC_WINDOW_MS = 30_000

export interface ChatSwitchResources {
  chatViewInstances: number
  cachedChatViews: number
  timelineDomNodes?: number
  markdownInstances?: number
  richMarkdownInstances?: number
  deferredFastPlanMessages?: number
  fastPlanSessions?: number
  activeFastPlanSessions?: number
}

export interface ChatSwitchSample {
  id: number
  fromPath: string
  toPath: string
  warm: boolean
  startedAt: number
  routeCommittedAt?: number
  viewActivatedAt?: number
  firstContentAt?: number
  fastPlanSnapshotAt?: number
  connectionReadyAt?: number
  hydrateStartedAt?: number
  hydrateFinishedAt?: number
  didReload: boolean
  messageCount: number
  planEntryCount: number
  clickToRouteMs?: number
  routeToActivationMs?: number
  activationToFirstContentMs?: number
  routeToFirstContentMs?: number
  clickToFirstContentMs?: number
  hydrateMs?: number
  reconnectMs?: number
  longTaskCount: number
  maxLongTaskMs: number
  resources: ChatSwitchResources
}

interface ChatSwitchSummaryBucket {
  count: number
  p50Ms: number | null
  p95Ms: number | null
}

export interface ChatSwitchPerfSnapshot {
  active: ChatSwitchSample | null
  samples: ChatSwitchSample[]
  summary: {
    all: ChatSwitchSummaryBucket
    warm: ChatSwitchSummaryBucket
    cold: ChatSwitchSummaryBucket
  }
  resources: ChatSwitchResources
}

type Clock = () => number

function percentile(values: number[], ratio: number): number | null {
  if (!values.length) return null
  const sorted = [...values].sort((a, b) => a - b)
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1)]
}

function summaryBucket(samples: ChatSwitchSample[]): ChatSwitchSummaryBucket {
  const values = samples
    .map(item => item.clickToFirstContentMs)
    .filter((value): value is number => typeof value === 'number')
  return {
    count: values.length,
    p50Ms: percentile(values, 0.5),
    p95Ms: percentile(values, 0.95),
  }
}

function cloneSample(sample: ChatSwitchSample | null): ChatSwitchSample | null {
  return sample ? { ...sample, resources: { ...sample.resources } } : null
}

/**
 * Low-overhead measurements for the user-visible chat route switch.
 *
 * Samples remain in memory only. Console output is opt-in through
 * localStorage[CHAT_SWITCH_PERF_DEBUG_KEY] = '1'.
 */
export class ChatSwitchPerfMonitor {
  private sequence = 0
  private active: ChatSwitchSample | null = null
  private samples: ChatSwitchSample[] = []
  private cachedPaths = new Set<string>()
  private chatViewPaths = new Map<string, number>()
  private observer: PerformanceObserver | null = null

  constructor(private readonly clock: Clock = () => performance.now()) {}

  begin(fromPath: string, toPath: string): number | null {
    if (!toPath || fromPath === toPath || !isChatPath(toPath)) return null
    if (this.active?.toPath === toPath) return this.active.id
    this.active = {
      id: ++this.sequence,
      fromPath,
      toPath,
      warm: this.cachedPaths.has(toPath),
      startedAt: this.clock(),
      didReload: false,
      messageCount: 0,
      planEntryCount: 0,
      longTaskCount: 0,
      maxLongTaskMs: 0,
      resources: this.currentResources(),
    }
    this.ensureLongTaskObserver()
    return this.active.id
  }

  ensure(fromPath: string, toPath: string) {
    if (!this.active || this.active.toPath !== toPath) this.begin(fromPath, toPath)
  }

  markRouteCommitted(path: string) {
    const sample = this.activeFor(path)
    if (!sample || sample.routeCommittedAt !== undefined) return
    sample.routeCommittedAt = this.clock()
    sample.clickToRouteMs = sample.routeCommittedAt - sample.startedAt
  }

  markViewActivated(path: string) {
    const sample = this.activeFor(path)
    if (!sample || sample.viewActivatedAt !== undefined) return
    sample.viewActivatedAt = this.clock()
    if (sample.routeCommittedAt !== undefined) {
      sample.routeToActivationMs = sample.viewActivatedAt - sample.routeCommittedAt
    }
  }

  markHydrateStarted(path: string) {
    const sample = this.activeFor(path)
    if (!sample) return
    sample.didReload = true
    sample.hydrateStartedAt ??= this.clock()
  }

  markHydrateFinished(path: string) {
    const sample = this.activeOrRecentFor(path)
    if (!sample || sample.hydrateFinishedAt !== undefined) return
    sample.hydrateFinishedAt = this.clock()
    if (sample.hydrateStartedAt !== undefined) {
      sample.hydrateMs = sample.hydrateFinishedAt - sample.hydrateStartedAt
    }
  }

  markFastPlanSnapshot(path: string) {
    const sample = this.activeOrRecentFor(path)
    if (sample && sample.fastPlanSnapshotAt === undefined) sample.fastPlanSnapshotAt = this.clock()
  }

  markConnectionReady(path: string) {
    const sample = this.activeOrRecentFor(path)
    if (!sample || sample.connectionReadyAt !== undefined) return
    sample.connectionReadyAt = this.clock()
    const base = sample.routeCommittedAt ?? sample.startedAt
    sample.reconnectMs = sample.connectionReadyAt - base
  }

  markFirstContent(
    path: string,
    details: {
      messageCount: number
      planEntryCount: number
      timelineDomNodes?: number
      markdownInstances?: number
      richMarkdownInstances?: number
      deferredFastPlanMessages?: number
      fastPlanSessions?: number
      activeFastPlanSessions?: number
      collectDomResources?: () => Pick<ChatSwitchResources,
        'timelineDomNodes' | 'markdownInstances' | 'richMarkdownInstances' | 'deferredFastPlanMessages'>
    },
  ) {
    const sample = this.activeFor(path)
    if (!sample || sample.firstContentAt !== undefined) return
    sample.firstContentAt = this.clock()
    sample.messageCount = details.messageCount
    sample.planEntryCount = details.planEntryCount
    sample.clickToFirstContentMs = sample.firstContentAt - sample.startedAt
    if (sample.viewActivatedAt !== undefined) {
      sample.activationToFirstContentMs = sample.firstContentAt - sample.viewActivatedAt
    }
    if (sample.routeCommittedAt !== undefined) {
      sample.routeToFirstContentMs = sample.firstContentAt - sample.routeCommittedAt
    }
    const domResources = details.collectDomResources?.() ?? {}
    sample.resources = {
      ...this.currentResources(),
      timelineDomNodes: domResources.timelineDomNodes ?? details.timelineDomNodes,
      markdownInstances: domResources.markdownInstances ?? details.markdownInstances,
      richMarkdownInstances: domResources.richMarkdownInstances ?? details.richMarkdownInstances,
      deferredFastPlanMessages: domResources.deferredFastPlanMessages ?? details.deferredFastPlanMessages,
      fastPlanSessions: details.fastPlanSessions,
      activeFastPlanSessions: details.activeFastPlanSessions,
    }
    this.samples.push(sample)
    if (this.samples.length > MAX_SAMPLES) this.samples.splice(0, this.samples.length - MAX_SAMPLES)
    this.active = null
    this.log(sample)
  }

  recordLongTask(startTime: number, duration: number) {
    const sample = this.active
    if (!sample || startTime < sample.startedAt) return
    sample.longTaskCount += 1
    sample.maxLongTaskMs = Math.max(sample.maxLongTaskMs, duration)
  }

  registerChatView(path: string) {
    this.chatViewPaths.set(path, (this.chatViewPaths.get(path) ?? 0) + 1)
  }

  unregisterChatView(path: string) {
    const count = this.chatViewPaths.get(path) ?? 0
    if (count <= 1) this.chatViewPaths.delete(path)
    else this.chatViewPaths.set(path, count - 1)
  }

  setCachedPaths(paths: Iterable<string>) {
    this.cachedPaths = new Set(paths)
  }

  reset() {
    this.active = null
    this.samples = []
  }

  snapshot(): ChatSwitchPerfSnapshot {
    const samples = this.samples.map(item => cloneSample(item)!)
    return {
      active: cloneSample(this.active),
      samples,
      summary: {
        all: summaryBucket(samples),
        warm: summaryBucket(samples.filter(item => item.warm)),
        cold: summaryBucket(samples.filter(item => !item.warm)),
      },
      resources: this.currentResources(),
    }
  }

  dispose() {
    this.observer?.disconnect()
    this.observer = null
  }

  private activeFor(path: string) {
    return this.active?.toPath === path ? this.active : null
  }

  private activeOrRecentFor(path: string) {
    const active = this.activeFor(path)
    if (active) return active
    const recent = [...this.samples].reverse().find(item => item.toPath === path)
    if (!recent) return null
    return this.clock() - recent.startedAt <= LATE_METRIC_WINDOW_MS ? recent : null
  }

  private currentResources(): ChatSwitchResources {
    return {
      chatViewInstances: [...this.chatViewPaths.values()].reduce((sum, count) => sum + count, 0),
      cachedChatViews: this.cachedPaths.size,
    }
  }

  private ensureLongTaskObserver() {
    if (this.observer || typeof PerformanceObserver === 'undefined') return
    try {
      this.observer = new PerformanceObserver(list => {
        for (const entry of list.getEntries()) this.recordLongTask(entry.startTime, entry.duration)
      })
      this.observer.observe({ type: 'longtask', buffered: true })
    } catch {
      this.observer = null
    }
  }

  private log(sample: ChatSwitchSample) {
    try {
      if (typeof localStorage === 'undefined' || localStorage.getItem(CHAT_SWITCH_PERF_DEBUG_KEY) !== '1') return
      console.info('[ChatSwitchPerf]', {
        fromPath: sample.fromPath,
        toPath: sample.toPath,
        warm: sample.warm,
        messageCount: sample.messageCount,
        planEntryCount: sample.planEntryCount,
        clickToRouteMs: sample.clickToRouteMs,
        routeToActivationMs: sample.routeToActivationMs,
        activationToFirstContentMs: sample.activationToFirstContentMs,
        routeToFirstContentMs: sample.routeToFirstContentMs,
        clickToFirstContentMs: sample.clickToFirstContentMs,
        hydrateMs: sample.hydrateMs,
        reconnectMs: sample.reconnectMs,
        didReload: sample.didReload,
        longTaskCount: sample.longTaskCount,
        maxLongTaskMs: sample.maxLongTaskMs,
        resources: sample.resources,
      })
    } catch {
      // Diagnostics must never affect navigation.
    }
  }
}

function isChatPath(path: string) {
  return path === '/chat' || path.startsWith('/chat/')
}

export const chatSwitchPerf = new ChatSwitchPerfMonitor()

export function installChatSwitchPerfDiagnostics() {
  if (typeof window === 'undefined') return
  ;(window as any).__FASTCLAW_CHAT_SWITCH_PERF__ = Object.freeze({
    snapshot: () => chatSwitchPerf.snapshot(),
    reset: () => chatSwitchPerf.reset(),
    enableDebug: () => localStorage.setItem(CHAT_SWITCH_PERF_DEBUG_KEY, '1'),
    disableDebug: () => localStorage.removeItem(CHAT_SWITCH_PERF_DEBUG_KEY),
  })
}

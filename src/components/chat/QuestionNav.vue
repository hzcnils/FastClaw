<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { List } from 'lucide-vue-next'
import type { Message } from '@/types'

const props = defineProps<{
  messages: Message[]
  messageListRef: HTMLElement | null
  /** 提问序号 → groupedMessages 组索引（与 ChatView 一致） */
  questionGroupIndices?: number[]
  maxHeight?: number
  navTop?: number
}>()

const emit = defineEmits<{
  'scrollTo': [index: number]
}>()

const { t } = useI18n()
/** 当前视口对应的提问（持久高亮） */
const activeIndex = ref(0)
const showAllPanel = ref(false)
const allQuestionsRef = ref<HTMLElement | null>(null)
let hidePanelTimer: ReturnType<typeof setTimeout> | null = null

function openAllPanel() {
  if (hidePanelTimer) {
    clearTimeout(hidePanelTimer)
    hidePanelTimer = null
  }
  showAllPanel.value = true
}

function scheduleCloseAllPanel() {
  if (hidePanelTimer) clearTimeout(hidePanelTimer)
  hidePanelTimer = setTimeout(() => {
    hidePanelTimer = null
    showAllPanel.value = false
  }, 120)
}

function closeAllPanel() {
  if (hidePanelTimer) {
    clearTimeout(hidePanelTimer)
    hidePanelTimer = null
  }
  showAllPanel.value = false
}

interface QuestionItem {
  index: number
  content: string
  fullContent: string
}

const questions = computed<QuestionItem[]>(() => {
  const items: QuestionItem[] = []
  const seenGroupIds = new Set<string | null>()
  let qNum = 0
  props.messages.forEach((msg) => {
    if (msg.role === 'user') {
      const gid = msg.group_id || null
      if (gid && seenGroupIds.has(gid)) {
        return
      }
      if (gid) seenGroupIds.add(gid)
      const full = (msg.content || '').replace(/\s+/g, ' ').trim()
      items.push({
        index: qNum,
        content: full.length > 80 ? full.substring(0, 80) + '...' : full,
        fullContent: full,
      })
      qNum++
    }
  })
  return items
})

const groupIndices = computed(() => {
  if (props.questionGroupIndices?.length) return props.questionGroupIndices
  return questions.value.map((_, i) => i)
})

// ─── 分页 ───
const ITEM_H = 28
const maxSlots = computed(() => {
  if (!props.maxHeight || props.maxHeight <= 0) return 8
  return Math.max(3, Math.floor((props.maxHeight - 36) / ITEM_H))
})
const pageSize = computed(() => Math.max(1, maxSlots.value - 2))
const totalPages = computed(() => Math.max(1, Math.ceil(questions.value.length / pageSize.value)))
const page = ref(1)

watch(
  () => questions.value.length,
  (len, old) => {
    if (len > (old ?? 0)) {
      page.value = totalPages.value
      activeIndex.value = Math.max(0, len - 1)
    } else if (page.value > totalPages.value) {
      page.value = totalPages.value
    }
    if (activeIndex.value >= len) {
      activeIndex.value = Math.max(0, len - 1)
    }
  },
)
watch(totalPages, (tp) => {
  if (page.value > tp) page.value = tp
})

const usePager = computed(() => questions.value.length > maxSlots.value)

const pageQuestions = computed<QuestionItem[]>(() => {
  if (!usePager.value) return questions.value
  const start = (page.value - 1) * pageSize.value
  return questions.value.slice(start, start + pageSize.value)
})

const maxHeightStyle = computed(() => {
  const top = (props.navTop || 0) + 'px'
  const h = (props.maxHeight && props.maxHeight > 0) ? props.maxHeight + 'px' : 'auto'
  return { top, height: h, overflow: 'visible' }
})

function ensurePageForIndex(index: number) {
  if (!usePager.value) return
  const targetPage = Math.floor(index / pageSize.value) + 1
  if (targetPage !== page.value) page.value = targetPage
}

function prevPage() {
  if (page.value > 1) page.value--
}
function nextPage() {
  if (page.value < totalPages.value) page.value++
}

let clickLockUntil = 0

function scrollToQuestion(index: number) {
  activeIndex.value = index
  clickLockUntil = Date.now() + 900
  ensurePageForIndex(index)
  emit('scrollTo', index)
  closeAllPanel()
}

function onDocumentPointerDown(e: PointerEvent) {
  if (!showAllPanel.value) return
  const root = allQuestionsRef.value
  if (root && e.target instanceof Node && root.contains(e.target)) return
  closeAllPanel()
}

function getScrollWrap(): HTMLElement | null {
  const root = props.messageListRef
  if (!root) return null
  return (root.querySelector('.el-scrollbar__wrap') as HTMLElement | null) || root
}

/** 根据滚动位置，高亮当前界面主要显示的提问 */
function updateActiveFromScroll() {
  if (Date.now() < clickLockUntil) return
  const listRoot = props.messageListRef
  const wrap = getScrollWrap()
  if (!listRoot || !wrap || questions.value.length === 0) return

  const wrapRect = wrap.getBoundingClientRect()
  if (wrapRect.height <= 0) return

  const markerY = wrapRect.top + Math.min(140, wrapRect.height * 0.18)
  const nearBottom = wrap.scrollTop + wrap.clientHeight >= wrap.scrollHeight - 24

  if (nearBottom) {
    const last = questions.value.length - 1
    if (activeIndex.value !== last) {
      activeIndex.value = last
      ensurePageForIndex(last)
    }
    return
  }

  let current = 0
  const indices = groupIndices.value
  for (let qi = 0; qi < questions.value.length; qi++) {
    const gi = indices[qi]
    if (gi === undefined) continue
    const el = listRoot.querySelector(`[data-group-index="${gi}"]`) as HTMLElement | null
    if (!el) continue
    const top = el.getBoundingClientRect().top
    if (top <= markerY) {
      current = qi
    } else if (qi === 0 && top > markerY) {
      current = 0
      break
    } else {
      break
    }
  }

  if (activeIndex.value !== current) {
    activeIndex.value = current
    ensurePageForIndex(current)
  }
}

let scrollEl: HTMLElement | null = null
let rafId = 0

function onScroll() {
  if (rafId) return
  rafId = requestAnimationFrame(() => {
    rafId = 0
    updateActiveFromScroll()
  })
}

function bindScroll() {
  unbindScroll()
  scrollEl = getScrollWrap()
  if (scrollEl) {
    scrollEl.addEventListener('scroll', onScroll, { passive: true })
    nextTick(() => updateActiveFromScroll())
  }
}

function unbindScroll() {
  if (scrollEl) {
    scrollEl.removeEventListener('scroll', onScroll)
    scrollEl = null
  }
  if (rafId) {
    cancelAnimationFrame(rafId)
    rafId = 0
  }
}

watch(
  () => props.messageListRef,
  () => {
    nextTick(() => bindScroll())
  },
)

watch(
  () => [props.messages.length, groupIndices.value.length] as const,
  () => {
    nextTick(() => updateActiveFromScroll())
  },
)

onMounted(() => {
  document.addEventListener('pointerdown', onDocumentPointerDown, true)
  nextTick(() => bindScroll())
})
onUnmounted(() => {
  document.removeEventListener('pointerdown', onDocumentPointerDown, true)
  if (hidePanelTimer) {
    clearTimeout(hidePanelTimer)
    hidePanelTimer = null
  }
  unbindScroll()
})
</script>

<template>
  <div v-if="questions.length > 1" class="question-nav-wrapper" :style="maxHeightStyle">
    <div
      ref="allQuestionsRef"
      class="all-questions"
      @mouseenter="openAllPanel"
      @mouseleave="scheduleCloseAllPanel"
    >
      <button
        type="button"
        class="all-questions-btn"
        :class="{ 'is-open': showAllPanel }"
        :aria-label="t('chat.allQuestions')"
        :aria-expanded="showAllPanel"
      >
        <List :size="15" stroke-width="2.2" />
      </button>

      <div v-show="showAllPanel" class="all-questions-panel" @click.stop>
        <button
          v-for="q in questions"
          :key="q.index"
          type="button"
          class="all-q-item"
          :class="{ 'is-active': activeIndex === q.index }"
          @click="scrollToQuestion(q.index)"
        >
          <span class="all-q-text">{{ q.content }}</span>
        </button>
      </div>
    </div>

    <div class="nav-items">
      <div v-if="usePager" class="nav-item nav-arrow" :class="{ 'is-disabled': page <= 1 }" @click="prevPage">
        <span class="nav-dot">↑</span>
      </div>
      <div
        v-for="q in pageQuestions"
        :key="q.index"
        class="nav-item"
        :class="{ 'is-active': activeIndex === q.index }"
        @click="scrollToQuestion(q.index)"
      >
        <span class="nav-dot">{{ q.index + 1 }}</span>
        <span class="nav-text" :title="q.fullContent">{{ q.content }}</span>
      </div>
      <div v-if="usePager" class="nav-item nav-arrow" :class="{ 'is-disabled': page >= totalPages }" @click="nextPage">
        <span class="nav-dot">↓</span>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.question-nav-wrapper {
  position: absolute;
  left: 4px;
  z-index: 10;
  overflow: visible;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  gap: 6px;

  &::-webkit-scrollbar {
    width: 0;
  }
}

.all-questions {
  position: relative;
  flex-shrink: 0;
  z-index: 12;
}

.all-questions-btn {
  width: 26px;
  height: 26px;
  padding: 0;
  border: none;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-secondary, #8a94a6);
  background: color-mix(in srgb, var(--bg-hover, #f0f0f0) 70%, transparent);
  transition: color 0.18s ease, background 0.18s ease, box-shadow 0.18s ease;

  &:hover,
  &.is-open {
    color: var(--color-primary, #409eff);
    background: color-mix(in srgb, var(--color-primary, #409eff) 12%, transparent);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  }
}

.all-questions-panel {
  position: absolute;
  left: calc(100% + 8px);
  top: 0;
  min-width: 168px;
  max-width: min(280px, 42vw);
  max-height: min(360px, 52vh);
  overflow-x: hidden;
  overflow-y: auto;
  padding: 8px 0;
  border-radius: 14px;
  background: var(--theme-dailog-color);
  border: 1px solid var(--border-color, rgba(0, 0, 0, 0.06));
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.12);
  z-index: 30;

  &::-webkit-scrollbar {
    width: 4px;
  }
  &::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.18);
    border-radius: 4px;
  }
  &::-webkit-scrollbar-track {
    background: transparent;
  }
}

.all-q-item {
  width: 100%;
  display: flex;
  align-items: center;
  padding: 8px 14px;
  border: none;
  background: transparent;
  cursor: pointer;
  text-align: left;
  color: var(--text-secondary, #9aa3b2);
  transition: color 0.15s ease, background 0.15s ease;

  .all-q-text {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    line-height: 1.35;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  &:hover {
    color: var(--color-primary, #409eff);
    background: color-mix(in srgb, var(--color-primary, #409eff) 6%, transparent);
  }

  &.is-active {
    color: var(--color-primary, #409eff);
    font-weight: 700;
  }
}

.nav-items {
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow: hidden;
  max-height: 100%;
}

.nav-item {
  display: flex;
  align-items: center;
  cursor: pointer;
  padding: 2px 6px 2px 2px;
  border-radius: 14px;
  background: transparent;
  transition: background 0.2s;
  max-width: 26px;

  &:hover {
    max-width: 220px;
    border-color: var(--color-primary);
    color: var(--color-primary);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
    background-color: var(--theme-card-bg);
  }

  &.is-active .nav-dot {
    background: var(--color-primary);
    color: #fff;
    border-color: var(--color-primary);
    box-shadow: 0 0 8px color-mix(in srgb, var(--color-primary) 40%, transparent);
  }
}

.nav-arrow {
  cursor: pointer;

  &.is-disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
}

.nav-dot {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--bg-hover, #f0f0f0) 60%, transparent);
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: background 0.2s, color 0.2s;
  user-select: none;

  .nav-item:hover & {
    background: #dde4fc;
    color: #409eff;
  }

  .nav-arrow & {
    font-size: 13px;
    font-weight: 700;
  }
}

.nav-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 0;
  margin-left: 0;
  font-size: 12px;
  color: var(--text-primary);
  transition: max-width 0.3s ease, margin-left 0.3s ease;
  user-select: none;

  .nav-item:hover & {
    max-width: 180px;
    margin-left: 6px;
  }
}
</style>

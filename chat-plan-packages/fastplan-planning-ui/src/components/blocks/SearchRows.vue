<script setup>
/**
 * SearchRows.vue — 行式网络搜索块
 * 父行不缩进、前置放大镜图标；每次调用一行，向后缩进；点行展开命中结果。
 *   正在执行网络内容搜索           命中 8 · 798ms
 *     › ✓ web_fetch  百度  关键词……   5 条 · 412ms
 */
import { ref, computed } from 'vue'
import { Search, Loader2, Check, ChevronRight } from 'lucide-vue-next'

const props = defineProps({
  calls: { type: Array, default: () => [] },   // [{tool, source, kw, results, done, ms}]
  phase: { type: String, default: 'in_node' }, // preflight | planned | in_node
  round: { type: Number, default: 0 },
  editable: { type: Boolean, default: false }, // 规划确认阶段允许改关键词
})
const emit = defineEmits(['update-kw'])

const open = ref({})
const folded = ref(false)

const label = computed(() => {
  if (props.phase === 'preflight') return '预检索探路'
  if (props.phase === 'planned') return props.calls.every(c => c.done) ? '联网搜索' : '计划检索'
  return props.round > 1 ? `迭代检索 · 第 ${props.round} 轮` : '正在执行网络内容搜索'
})
const allDone = computed(() => props.calls.length > 0 && props.calls.every(c => c.done))
const hits = computed(() => props.calls.reduce((s, c) => s + (c.results?.length || 0), 0))
const totalMs = computed(() => props.calls.reduce((s, c) => s + (c.ms || 0), 0))

const onKw = (i, e) => emit('update-kw', { index: i, value: e.target.innerText.trim() })
</script>

<template>
  <div class="fp-search">
    <!-- 父行：不缩进 -->
    <div class="flex items-center gap-1.5 text-[11px]">
      <Search :size="12" class="text-sky-500 shrink-0" />
      <span class="font-medium text-slate-700">{{ label }}</span>
      <span v-if="allDone" class="font-mono text-[10px] text-slate-300">
        命中 {{ hits }}<span v-if="totalMs"> · {{ totalMs }}ms</span>
      </span>
      <span class="flex-1"></span>
      <button v-if="allDone" @click="folded = !folded"
        class="text-[10px] text-slate-400 hover:text-indigo-600">{{ folded ? '展开' : '折叠' }}</button>
    </div>

    <!-- 子行：向后缩进 -->
    <div v-show="!folded" class="fp-indent mt-0.5 space-y-px">
      <div v-for="(c, i) in calls" :key="i">
        <button class="w-full flex items-center gap-1.5 text-[11px] text-left py-px rounded hover:bg-slate-50"
                @click="open[i] = !open[i]">
          <ChevronRight :size="10" class="text-slate-300 shrink-0 transition-transform"
                        :class="{ 'rotate-90': open[i] }" />
          <Loader2 v-if="!c.done" :size="11" class="text-indigo-500 animate-spin shrink-0" />
          <Check v-else :size="11" class="text-emerald-500 shrink-0" />
          <span class="font-mono text-violet-600 shrink-0">{{ c.tool }}</span>
          <span class="text-[10px] px-1 rounded bg-sky-50 text-sky-700 shrink-0">{{ c.source }}</span>
          <span class="text-slate-600 truncate flex-1 rounded px-0.5"
                :contenteditable="editable" spellcheck="false"
                :class="editable ? 'bg-indigo-50 ring-1 ring-indigo-200 outline-none' : ''"
                @click.stop @blur="onKw(i, $event)">{{ c.kw }}</span>
          <span class="font-mono text-[10px] text-slate-300 shrink-0">
            <template v-if="c.done">{{ c.results?.length || 0 }} 条<span v-if="c.ms"> · {{ c.ms }}ms</span></template>
            <template v-else>检索中</template>
          </span>
        </button>

        <div v-show="open[i]" class="ml-8 space-y-px pb-1">
          <div v-for="(r, ri) in (c.results || [])" :key="ri"
               class="flex items-baseline gap-1.5 text-[10px]">
            <span class="font-mono text-indigo-400">[{{ ri + 1 }}]</span>
            <a v-if="r.url" :href="r.url" target="_blank" rel="noopener"
               class="text-slate-500 truncate hover:text-indigo-600 hover:underline">{{ r.title }}</a>
            <span v-else class="text-slate-500 truncate">{{ r.title }}</span>
            <span v-if="r.source" class="font-mono text-[10px] text-sky-600 shrink-0">{{ r.source }}</span>
          </div>
          <div v-if="!(c.results || []).length" class="text-[10px] text-slate-300">暂无结构化结果</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 唯一的层级线索：缩进 + 一条发丝竖线 */
.fp-indent { margin-left: 6px; padding-left: 14px; border-left: 1px solid #eef2f6; }
</style>

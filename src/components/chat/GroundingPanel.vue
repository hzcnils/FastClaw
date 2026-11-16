<!--
  证据台账面板 —— 回答里的数字能不能追溯到某次真实的工具返回。

  背景：2026-09-23 甲方现场，问两个相距 800 公里的城市天气，拿到逐字相同的一组数据。
  系统当时既没有检查"这些数字是哪来的"，也没有在任何地方告诉用户"没查到"。
  这个面板就是那个缺失的出口。

  设计约束（与 Rust 侧 `core::grounding` 一一对应，改一边必须改另一边）：
  - 提示语由**后端**生成后传下来（`note`），前端不重新判定——两边算法一漂移，
    界面显示的和实际判定的就会打架。
  - 状态串沿用 FastPlan 规划链的 grounded/partial/ungrounded/skipped，
    两条链共用同一套词汇，用户看到的才是同一个系统。
-->
<template>
  <div v-if="hasContent" class="grounding-panel">
    <!-- 系统提示语：不经过模型，模型无法用任何措辞让它消失 -->
    <div v-if="note" class="grounding-note">
      <span class="grounding-note-icon">⚠️</span>
      <span class="grounding-note-text">{{ note }}</span>
    </div>

    <!-- 证据明细：默认折叠，点开才看 -->
    <div v-if="sources.length" class="grounding-evidence">
      <button class="grounding-toggle" @click="expanded = !expanded">
        <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"
          class="grounding-arrow" :class="{ expanded }">
          <polyline points="9 18 15 12 9 6" />
        </svg>
        <span>{{ toggleLabel }}</span>
      </button>

      <div v-if="expanded" class="grounding-detail">
        <div v-for="src in sources" :key="src.index" class="grounding-source"
          :class="{ failed: !src.success }">
          <div class="grounding-source-head">
            <span class="grounding-source-index">[{{ src.index }}]</span>
            <span class="grounding-source-tool">{{ src.tool }}</span>
            <span v-if="!src.success" class="grounding-source-status">未成功</span>
          </div>
          <div v-if="src.error" class="grounding-source-error">{{ src.error }}</div>
          <div v-else class="grounding-source-excerpt">{{ src.excerpt }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { GroundingManifest } from '@/types'

const props = defineProps<{
  grounding?: GroundingManifest
  note?: string
}>()

const expanded = ref(false)

const sources = computed(() => props.grounding?.sources ?? [])

const hasContent = computed(() => !!props.note || sources.value.length > 0)

/** 「证据 3 条 · 2/5 个值可溯源」——数字直接来自台账，不做二次加工 */
const toggleLabel = computed(() => {
  const g = props.grounding
  if (!g) return `证据 ${sources.value.length} 条`
  const traceable = g.claims.filter(c => c.sources.length > 0).length
  const total = g.claims.length
  if (total === 0) return `证据 ${sources.value.length} 条`
  return `证据 ${sources.value.length} 条 · ${traceable}/${total} 个值可溯源`
})
</script>

<style scoped>
.grounding-panel {
  margin-top: 10px;
}

.grounding-note {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 12px;
  line-height: 1.6;
  background: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.25);
  color: var(--text-primary, #1f2937);
}

.grounding-note-icon {
  flex-shrink: 0;
}

.grounding-note-text {
  white-space: pre-wrap;
  word-break: break-word;
}

.grounding-evidence {
  margin-top: 6px;
}

.grounding-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary, #6b7280);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;
}

.grounding-toggle:hover {
  background: var(--bg-hover, #f3f4f6);
  color: var(--text-primary, #1f2937);
}

.grounding-arrow {
  transition: transform 0.2s;
}

.grounding-arrow.expanded {
  transform: rotate(90deg);
}

.grounding-detail {
  margin-top: 6px;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--bg-hover, #f9fafb);
  border: 1px solid var(--border-color, #e5e7eb);
}

.grounding-source + .grounding-source {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--border-color, #e5e7eb);
}

.grounding-source-head {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-secondary, #6b7280);
}

.grounding-source-index {
  font-family: ui-monospace, monospace;
  opacity: 0.7;
}

.grounding-source-tool {
  font-weight: 600;
  color: var(--text-primary, #1f2937);
}

.grounding-source-status {
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 11px;
  background: rgba(239, 68, 68, 0.1);
  color: #dc2626;
}

.grounding-source-excerpt,
.grounding-source-error {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #6b7280);
  white-space: pre-wrap;
  word-break: break-word;
  /* 证据原文可能很长，最多铺 4 行，细节交给折叠 */
  display: -webkit-box;
  -webkit-line-clamp: 4;
  line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.grounding-source-error {
  color: #dc2626;
}
</style>

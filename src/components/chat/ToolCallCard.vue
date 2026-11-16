<script setup lang="ts">
import { ref, computed } from 'vue'
import { Check, X, ChevronDown, Loader2 } from 'lucide-vue-next'
import type { ToolCallEvent } from '@/types'
import { isWebSearchTool } from '@/utils/toolName'
import SearchResultsContent from '@/components/chat/SearchResultsContent.vue'

const props = defineProps<{
  tool: ToolCallEvent
}>()

const expanded = ref(false)

// 联网搜索工具：done 状态整体委托 SearchResultsContent（折叠列表 + 右侧 iframe 预览）
const isSearch = computed(() => isWebSearchTool(props.tool.name))

const kindLabel = computed(() => {
  const k = props.tool.kind || 'builtin'
  if (k === 'mcp') {
    const parts = props.tool.name.split('__')
    return parts.length >= 2 ? `MCP · ${parts[1]}` : 'MCP'
  }
  if (k === 'skill') return 'Skill'
  return '工具'
})

const kindClass = computed(() => {
  const k = props.tool.kind || 'builtin'
  if (k === 'mcp') return 'kind-mcp'
  if (k === 'skill') return 'kind-skill'
  return 'kind-builtin'
})

const hasPreview = computed(() => {
  return props.tool.result_preview && props.tool.result_preview.length > 0
})
</script>

<template>
  <!-- 联网搜索：done → 委托 SearchResultsContent；running/error 仍走下方状态卡片 -->
  <SearchResultsContent v-if="isSearch && tool.status === 'done'" :tool="tool" />

  <div v-else class="tool-call-card" :class="[`status-${tool.status}`, kindClass]">
    <!-- Running -->
    <template v-if="tool.status === 'running'">
      <span class="tc-spinner"><Loader2 :size="13" class="animate-spin" /></span>
      <span class="tc-label">{{ kindLabel }}</span>
      <span class="tc-name">{{ tool.name }}</span>
      <span class="tc-state running-text">执行中…</span>
    </template>

    <!-- Done（非搜索工具） -->
    <template v-else-if="tool.status === 'done'">
      <span class="tc-icon done-icon"><Check :size="14" /></span>
      <span class="tc-label">{{ kindLabel }}</span>
      <span class="tc-name">{{ tool.name }}</span>
      <span class="tc-state done-text">完成</span>
      <button
        v-if="hasPreview"
        class="tc-expand"
        @click="expanded = !expanded"
        :title="expanded ? '收起' : '展开结果'"
      >
        <ChevronDown :size="12" :class="{ rotated: expanded }" />
      </button>
    </template>

    <!-- Error -->
    <template v-else>
      <span class="tc-icon error-icon"><X :size="14" /></span>
      <span class="tc-label">{{ kindLabel }}</span>
      <span class="tc-name">{{ tool.name }}</span>
      <span class="tc-state error-text">失败</span>
      <button
        v-if="hasPreview"
        class="tc-expand"
        @click="expanded = !expanded"
        :title="expanded ? '收起' : '展开结果'"
      >
        <ChevronDown :size="12" :class="{ rotated: expanded }" />
      </button>
    </template>

    <!-- 非搜索工具：Result preview (expandable) -->
    <div v-if="expanded && hasPreview" class="tc-preview">
      <pre>{{ tool.result_preview }}</pre>
    </div>
  </div>
</template>

<style scoped>
.tool-call-card {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  margin: 4px 0;
  border-radius: 8px;
  font-size: 12px;
  line-height: 1.4;
  flex-wrap: wrap;
}
.tool-call-card.status-running {
  background: rgba(59, 130, 246, 0.06);
  border: 1px solid rgba(59, 130, 246, 0.18);
}
.tool-call-card.status-done {
  background: rgba(16, 185, 129, 0.05);
  border: 1px solid rgba(16, 185, 129, 0.15);
}
.tool-call-card.status-error {
  background: rgba(239, 68, 68, 0.05);
  border: 1px solid rgba(239, 68, 68, 0.15);
}

.tc-spinner,
.tc-icon {
  flex-shrink: 0;
  display: flex;
  align-items: center;
}
.done-icon { color: #10b981; }
.error-icon { color: #ef4444; }
.running-text { color: #3b82f6; }
.done-text { color: #10b981; }
.error-text { color: #ef4444; }

.tc-label {
  font-weight: 600;
  font-size: 11px;
  padding: 1px 5px;
  border-radius: 4px;
  flex-shrink: 0;
}
.kind-builtin .tc-label { background: rgba(59, 130, 246, 0.1); color: var(--special-text-color); }
.kind-mcp .tc-label { background: rgba(139, 92, 246, 0.1); color: #8b5cf6; }
.kind-skill .tc-label { background: rgba(245, 158, 11, 0.1); color: #d97706; }

.tc-name {
  font-family: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace;
  color: var(--text-primary);
  font-size: 11px;
}

.tc-state {
  font-size: 11px;
  font-weight: 500;
  flex-shrink: 0;
}

.tc-expand {
  margin-left: auto;
  padding: 1px 4px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--text-secondary);
  border-radius: 4px;
}
.tc-expand:hover { background: rgba(0, 0, 0, 0.06); }
.rotated { transform: rotate(180deg); }

.tc-preview {
  width: 100%;
  margin-top: 4px;
}
.tc-preview pre {
  font-family: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace;
  font-size: 11px;
  line-height: 1.4;
  background: rgba(0, 0, 0, 0.04);
  border-radius: 6px;
  padding: 8px 10px;
  max-height: 180px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
}
</style>

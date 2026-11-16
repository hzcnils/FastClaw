<script setup lang="ts">
// FastClaw · Command palette popup for skill/MCP selection in chat input
import { Search, X, Puzzle, Server, Layers } from 'lucide-vue-next'
import { ref, watch, onUnmounted } from 'vue'
import type { SlashCommand } from '@/composables/useSlashCommands'

const props = defineProps<{
  commands: SlashCommand[]
  selectedIndex: number
  visible: boolean
  searchQuery: string
  activeTab: 'all' | 'skill' | 'mcp'
}>()

const emit = defineEmits<{
  select: [cmd: SlashCommand]
  'update:searchQuery': [value: string]
  'update:activeTab': [value: 'all' | 'skill' | 'mcp']
  'update:selectedIndex': [value: number]
  close: []
}>()

const paletteRef = ref<HTMLElement | null>(null)

function handleClickOutside(e: MouseEvent) {
  if (paletteRef.value && !paletteRef.value.contains(e.target as Node)) {
    emit('close')
  }
}

watch(() => props.visible, (visible) => {
  if (visible) {
    document.addEventListener('click', handleClickOutside)
  } else {
    document.removeEventListener('click', handleClickOutside)
  }
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
})

const filterBtns = [
  { key: 'all' as const, icon: Layers, label: '全部' },
  { key: 'skill' as const, icon: Puzzle, label: 'Skills' },
  { key: 'mcp' as const, icon: Server, label: 'MCP' },
]

const typeBadge = (t: string) => {
  if (t === 'skill') return { text: 'Skill', class: 'badge-skill' }
  if (t === 'mcp') return { text: 'MCP', class: 'badge-mcp' }
  return null
}

const typeIcon = (t: string) => {
  if (t === 'skill') return Puzzle
  return Server
}

function onSearchInput(e: Event) {
  emit('update:searchQuery', (e.target as HTMLInputElement).value)
}

function onSearchKeydown(e: KeyboardEvent) {
  if (!props.commands.length) return
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    emit('update:selectedIndex', props.selectedIndex < props.commands.length - 1 ? props.selectedIndex + 1 : 0)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    emit('update:selectedIndex', props.selectedIndex > 0 ? props.selectedIndex - 1 : props.commands.length - 1)
  } else if (e.key === 'Enter') {
    e.preventDefault()
    const idx = props.selectedIndex >= 0 ? props.selectedIndex : 0
    if (props.commands[idx]) emit('select', props.commands[idx])
  }
}

function clearSearch() {
  emit('update:searchQuery', '')
}
</script>

<template>
  <div v-if="visible" ref="paletteRef" class="command-palette">
    <!-- 搜索栏 -->
    <div class="search-bar">
      <Search :size="14" class="search-icon" />
      <input
        ref="searchInputRef"
        :value="searchQuery"
        type="text"
        class="search-input"
        placeholder="搜索 Skill 或 MCP 工具..."
        @input="onSearchInput"
        @keydown="onSearchKeydown"
      />
      <div class="filter-btns">
        <button
          v-for="btn in filterBtns"
          :key="btn.key"
          class="filter-btn"
          :class="{ active: activeTab === btn.key }"
          :title="btn.label"
          @click="emit('update:activeTab', btn.key)"
        >
          <component :is="btn.icon" :size="13" />
          <span>{{ btn.label }}</span>
        </button>
      </div>
      <button v-if="searchQuery" class="search-clear" @click="clearSearch">
        <X :size="14" />
      </button>
    </div>

    <!-- 列表 -->
    <div class="list" v-if="commands.length">
      <div
        v-for="(cmd, i) in commands"
        :key="cmd.name"
        class="cmd-item"
        :class="{ selected: i === selectedIndex }"
        @click="emit('select', cmd)"
        @mouseenter="emit('update:selectedIndex', i)"
      >
        <!-- 左侧图标 -->
        <div class="cmd-icon" :class="cmd.type === 'skill' ? 'icon-skill' : 'icon-mcp'">
          <component :is="typeIcon(cmd.type)" :size="14" />
        </div>

        <!-- 中间信息 -->
        <div class="cmd-body">
          <div class="cmd-name">{{ cmd.labelEn }}</div>
          <div class="cmd-sub">{{ cmd.type === 'skill' ? cmd.labelZh : `${cmd.labelZh}` }}</div>
        </div>

        <!-- 右侧类型标签 -->
        <span v-if="typeBadge(cmd.type)" class="cmd-badge" :class="typeBadge(cmd.type)!.class">
          {{ typeBadge(cmd.type)!.text }}
        </span>
      </div>
    </div>

    <!-- 空状态 -->
    <div v-else class="list">
      <div class="empty-state">
        <Search :size="20" class="empty-icon" />
        <span>无匹配结果</span>
      </div>
    </div>



  </div>
</template>

<style scoped>
.command-palette {
  position: absolute;
  bottom: 100%;
  left: 0;
  right: 0;
  margin-bottom: 8px;
  background: var(--theme-dailog-color);
  border: 1px solid var(--border-color, #e5e7eb);
  border-radius: 14px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.10);
  z-index: 100;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* ── 搜索栏 ── */
.search-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-color, #f0f0f0);
}
.search-icon {
  color: var(--text-secondary, #9ca3af);
  flex-shrink: 0;
}
.search-input {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  font-size: 13px;
  color: var(--text-primary, #1f2937);
  min-width: 0;
}
.search-input::placeholder {
  color: var(--text-secondary, #9ca3af);
}
.filter-btns {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}
.filter-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary, #9ca3af);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}
.filter-btn:hover {
  color: var(--text-primary, #374151);
}
.filter-btn.active {
  color: var(--color-primary, #2f54eb);
}

.search-clear {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 50%;
  background: var(--bg-hover, #e5e7eb);
  color: var(--text-secondary, #9ca3af);
  cursor: pointer;
  flex-shrink: 0;
  padding: 0;
}
.search-clear:hover {
  background: #d1d5db;
  color: var(--text-primary, #374151);
}

/* ── 列表 ── */
.list {
  max-height: 260px;
  overflow-y: auto;
  padding: 4px;
}

/* ── 列表项 ── */
.cmd-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.12s;
}
.cmd-item:hover {
  background: var(--dropdown-hover-color);
}
.cmd-item.selected {
   background: var(--dropdown-hover-color);
}

/* ── 图标 ── */
.cmd-icon {
  width: 30px;
  height: 30px;
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.icon-skill {
  background: #f5f3ff;
  color: #7c3aed;
}
.icon-mcp {
  background: #ecfeff;
  color: #0891b2;
}

/* ── 主体信息 ── */
.cmd-body {
  flex: 1;
  min-width: 0;
}
.cmd-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary, #1f2937);
  line-height: 1.3;
}
.cmd-sub {
  font-size: 11px;
  color: var(--text-secondary, #9ca3af);
  line-height: 1.3;
  margin-top: 1px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── 类型标签 ── */
.cmd-badge {
  font-size: 10px;
  padding: 2px 7px;
  border-radius: 999px;
  font-weight: 600;
  white-space: nowrap;
  flex-shrink: 0;
  letter-spacing: 0.02em;
}
.badge-skill {
  background: #ede9fe;
  color: #7c3aed;
}
.badge-mcp {
  background: #cffafe;
  color: #0891b2;
}

/* ── 空状态 ── */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 32px 16px;
  color: var(--text-secondary, #9ca3af);
  font-size: 13px;
}
.empty-icon {
  opacity: 0.4;
}


</style>

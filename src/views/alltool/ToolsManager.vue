<script setup>
import { ref, computed, onMounted } from 'vue'
import {
  Plus, Trash2, Search, RefreshCw, X, Check, Loader2,
  ChevronDown, Wrench, Globe, Zap, Lock, ToggleLeft, ToggleRight, Box, List, Plug
} from 'lucide-vue-next'
import { useRouter } from 'vue-router'
import { toolsApi } from '../mcp/adminApi.js'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'

function errMsg(e) {
  return (typeof e === 'string' && e) || e?.message || (e != null ? String(e) : '未知错误')
}
import { useThemeStore } from '@/stores/theme'
import SearchClearButton from '@/components/common/SearchClearButton.vue'
import sdIcon from '@/assets/skills/ty.svg'
import wlIcon from '@/assets/skills/wl.svg'
import sdIconWhite from '@/assets/skills/ty_white.svg'
import wlIconWhite from '@/assets/skills/wl_white.svg'

const { t } = useI18n()
const themeStore = useThemeStore()
const router = useRouter()

const cardBgStyle = computed(() => {
  const id = themeStore.currentPreset.id
  console.log('[cardBgStyle] id =', id)
  if (id === 'simple-dark' || id.startsWith('dark-')) return { 'background-color': 'rgba(255,255,255,0.10)' }
  if (id === 'scene-graphite-cyan') return { 'background-color': 'rgba(0,0,0,0.10)' }
  if (id.startsWith('scene-')) return { 'background-color': 'rgba(255,255,255,0.10)' }
  if (id.startsWith('simple-light')) return { 'background-color': 'red' }
  console.log('[cardBgStyle] no match, returning {}')
  return {}
})

const toggleStyle = computed(() => {
  const id = themeStore.currentPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { 'background-color': 'rgba(0,0,0,0.3)' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { 'background-color': 'rgba(0,0,0,0.05)' }
  return {}
})

const toggleActiveStyle = computed(() => {
  const id = themeStore.currentPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { 'background-color': 'rgba(255,255,255,0.20)', color: '#fff' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { 'background-color': 'rgba(0,0,0,0.05)', color: '#1f2937' }
  return {}
})

const toggleInactiveStyle = computed(() => {
  const id = themeStore.currentPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { color: 'rgba(255,255,255,0.5)' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { 'background-color': 'rgba(0,0,0,0.05)', color: '#1f2937' }
  return {}
})

const filterBtnStyle = computed(() => {
const id = themeStore.currentPreset.id
  if (id === 'simple-light' || id == 'scene-graphite-cyan')
    return { color: '#282c34', border: '1px solid rgba(0,0,0,0.1)', 'border-radius': '4px' }
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { color: '#cbd5e0', border: '1px solid rgba(255,255,255,0.12)', 'border-radius': '4px' }
  if (id.startsWith('scene-') )
    return { color: '#fff', border: '1px solid rgba(255,255,255,0.20)', 'border-radius': '4px' }
  return {}
})

const filterMenuStyle = computed(() => {
  const id = themeStore.currentPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return {background: 'var(--theme-dailog-color)', border: '1px solid rgba(255,255,255,0.1)' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { background: 'var(--theme-dailog-color)' }
  return { background: 'var(--theme-dailog-color)' }
})
const filterMenuItemStyle = computed(() => {
  const id = themeStore.currentPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { color: '#fff' }
  return {}
})

const toolsTableHeadRef = ref(null)
const toolsTableBodyRef = ref(null)

function onToolsTableBodyScroll() {
  const head = toolsTableHeadRef.value
  const body = toolsTableBodyRef.value
  if (head && body) head.scrollLeft = body.scrollLeft
}

const items = ref([])
const loading = ref(true)
const error = ref('')
const viewMode = ref('list')
const searchQuery = ref('')
const typeFilter = ref('all')
const statusFilter = ref('all')
const showTypeMenu = ref(false)
const showStatusMenu = ref(false)
const showCreateMenu = ref(false)
const showHttpDialog = ref(false)
const showSkillDialog = ref(false)

// HTTP tool form
const httpForm = ref({
  name: '', display_name: '', description: '',
  method: 'GET', url: '', headers: [], schema: ''
})

// Skill form
const skillForm = ref({
  name: '', description: '', category: 'general', version: '1.0.0', code: '# Python\ndef run(params):\n    pass\n'
})

const creating = ref(false)

const typeOptions = computed(() => [
  { value: 'all', label: t('tools.allTypes') },
  { value: 'MCP', label: 'MCP' },
  { value: 'HTTP', label: 'HTTP' },
  { value: 'SKILL', label: 'Skill' },
])

const statusOptions = computed(() => [
  { value: 'all', label: t('tools.allStatus') },
  { value: 'active', label: t('tools.enable') },
  { value: 'inactive', label: t('tools.disable') },
])

const filtered = computed(() => {
  let result = items.value
  if (typeFilter.value !== 'all') result = result.filter(i => i.tool_type === typeFilter.value)
  if (statusFilter.value === 'active') result = result.filter(i => i.is_active)
  if (statusFilter.value === 'inactive') result = result.filter(i => !i.is_active)
  if (searchQuery.value) {
    const q = searchQuery.value.toLowerCase()
    result = result.filter(i => i.name?.toLowerCase().includes(q) || i.display_name?.toLowerCase().includes(q) || i.description?.toLowerCase().includes(q))
  }
  return result
})

// 分页
const currentPage = ref(1)
const pageSize = ref(15)
const totalPages = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pagedItems = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  return filtered.value.slice(start, start + pageSize.value)
})
const pageRange = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value + 1
  const end = Math.min(currentPage.value * pageSize.value, filtered.value.length)
  return { start, end, total: filtered.value.length }
})

const stats = computed(() => ({
  total: items.value.length,
  active: items.value.filter(i => i.is_active).length,
  categories: new Set(items.value.map(i => i.tool_type)).size,
}))

function typeBadgeClass(type) {
  if (type === 'MCP') return 'badge-mcp'
  if (type === 'HTTP') return 'badge-http'
  return 'badge-skill'
}

// 检查是否使用白色图标（黑暗主题或情景主题除雾灰调）
function useWhiteIcon() {
  const id = themeStore.currentPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-')) return true
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan') return true
  return false
}

function typeIcon(type) {
  const isWhite = useWhiteIcon()
  if (type === 'HTTP') return isWhite ? wlIconWhite : wlIcon
  if (type === 'SKILL') return isWhite ? sdIconWhite : sdIcon
  return '🔌'
}
function isSvgIcon(type) {
  return type === 'HTTP' || type === 'SKILL'
}

function sourceLabel(item) {
  if (item.tool_type === 'MCP') return item.source_server || 'MCP Server'
  return t('tools.manualCreate')
}

const apiReady = ref(true)

async function load() {
  loading.value = true; error.value = ''
  try {

    const res = await toolsApi.list()
    items.value = res.items || res || []
    apiReady.value = true
  } catch (e) {
    error.value = errMsg(e)
  }
  finally { loading.value = false }
}

async function toggleActive(item) {
  try {
    await toolsApi.toggle(item.id, !item.is_active)
    item.is_active = !item.is_active
  } catch (e) {
    error.value = errMsg(e)
    ElMessage.error(errMsg(e))
  }
}

async function remove(item) {
  if (item.tool_type === 'MCP') return
  try {
    await ElMessageBox.confirm(
      t('tools.confirmDeleteTool', { name: item.display_name || item.name }),
      t('common.delete'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
        customClass: 'delete-confirm-dialog',
        cancelButtonClass: 'delete-confirm-cancel-btn',
        closeOnClickModal: false,
      }
    )
  } catch { return }
  try {
    await toolsApi.delete(item.id)
    items.value = items.value.filter(i => i.id !== item.id)
  } catch (e) {
    error.value = errMsg(e)
    ElMessage.error(errMsg(e))
  }
}

function addHeader() { httpForm.value.headers.push({ key: '', value: '' }) }
function removeHeader(i) { httpForm.value.headers.splice(i, 1) }

async function createHttpTool() {
  creating.value = true
  try {
    let schema = {}
    const rawSchema = httpForm.value.schema.trim()
    if (rawSchema) {
      try { schema = JSON.parse(rawSchema) } catch { }
    } else {
      schema = { type: 'object', properties: {} }
    }
    const headers = httpForm.value.headers.filter(h => h.key.trim())
      .reduce((acc, h) => { acc[h.key.trim()] = h.value; return acc }, {})
    await toolsApi.create({
      name: httpForm.value.name,
      display_name: httpForm.value.display_name || httpForm.value.name,
      description: httpForm.value.description,
      tool_type: 'HTTP',
      input_schema: schema,
      config: { method: httpForm.value.method, url: httpForm.value.url, headers },
    })
    showHttpDialog.value = false
    httpForm.value = { name: '', display_name: '', description: '', method: 'GET', url: '', headers: [], schema: '' }
    await load()
  } catch (e) {
    error.value = errMsg(e)
    ElMessage.error(errMsg(e))
  }
  finally { creating.value = false }
}

async function createSkillTool() {
  creating.value = true
  try {
    await toolsApi.create({
      name: skillForm.value.name,
      display_name: skillForm.value.name,
      description: skillForm.value.description,
      tool_type: 'SKILL',
      config: { category: skillForm.value.category, version: skillForm.value.version, code: skillForm.value.code },
    })
    showSkillDialog.value = false
    skillForm.value = { name: '', description: '', category: 'general', version: '1.0.0', code: '# Python\ndef run(params):\n    pass\n' }
    await load()
  } catch (e) {
    error.value = errMsg(e)
    ElMessage.error(errMsg(e))
  }
  finally { creating.value = false }
}
 
onMounted(load)
</script>

<template>
  <div class="tools-page mx-auto h-full w-full flex flex-col overflow-hidden transition-all duration-300" :class="['px-3 py-7', 'xl:px-6', '2xl:px-[20px]']"
    :data-theme-id="themeStore.currentPreset.id"
    style="background: var(--bg-primary);"
    @click="showTypeMenu = false; showStatusMenu = false; showCreateMenu = false">

    <!-- Header -->
    <div class="flex items-start justify-between mb-3.5">
      <div>
        <div class="tools-title text-lg font-bold">{{ $t('tools.title') }}</div>
        <div class="tools-subtitle text-base mt-1">{{ $t('tools.subtitle') }}</div>
      </div>
      <div class="flex items-center gap-3">
        <div class="relative">
          <button @click.stop="showCreateMenu = !showCreateMenu"
            style="background: var(--btn-primary-bg) !important"
           @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
           @mouseleave="$event.currentTarget.style.background='var(--color-primary)'"
            class="px-5 py-2.5 text-sm font-bold rounded text-white shadow-lg transition-all flex items-center gap-2">
            <Plus :size="16" /> {{ $t('tools.newApi') }}
            <ChevronDown :size="14" class="opacity-80" />
          </button>

          <!-- Dropdown Menu -->
          <div v-if="showCreateMenu"
            class="tools-create-menu absolute right-0 mt-2 w-56 bg-white  rounded-lg shadow-[0_10px_30px_-10px_rgba(0,0,0,0.1)] z-20 overflow-hidden animate-in fade-in slide-in-from-top-2 duration-200"
            style="background: var(--theme-dailog-color) !important;">
            <div class="p-1.5 space-y-0.5">
              <button @click="showHttpDialog = true; showCreateMenu = false"
                class="w-full text-left px-3 py-2.5 text-sm text-[var(--text-primary)] rounded-lg flex items-center gap-3 transition-colors group hover:bg-[var(--dropdown-hover-color)]">
                <div
                  class="create-menu-icon w-8 h-8 rounded-lg bg-emerald-50 text-emerald-600 flex items-center justify-center group-hover:bg-emerald-100 transition-colors">
                  <Globe :size="16" />
                </div>
                <div>
                  <div class="font-medium">{{ $t('tools.restfulLabel') }}</div>
                  <div class="text-[var(--font-size-xs)] text-gray-400">{{ $t('tools.restfulDesc') }}</div>
                </div>
              </button>
              <button @click="showSkillDialog = true; showCreateMenu = false"
                class="w-full text-left px-3 py-2.5 text-sm text-[var(--text-primary)] rounded-lg flex items-center gap-3 transition-colors group hover:bg-[var(--dropdown-hover-color)]">
                <div
                  class="create-menu-icon w-8 h-8 rounded-lg bg-violet-50 text-violet-600 flex items-center justify-center group-hover:bg-violet-100 transition-colors">
                  <Zap :size="16" />
                </div>
                <div>
                  <div class="font-medium">{{ $t('tools.localSkill') }}</div>
                  <div class="text-[var(--font-size-xs)] text-gray-400" >{{ $t('tools.localSkillDesc') }}</div>
                </div>
              </button>
              <button @click="router.push('/mcp'); showCreateMenu = false"
                class="w-full text-left px-3 py-2.5 text-sm text-[var(--text-primary)] rounded-lg flex items-center gap-3 transition-colors group hover:bg-[var(--dropdown-hover-color)]">
                <div
                  class="create-menu-icon w-8 h-8 rounded-lg bg-blue-50 text-blue-600 flex items-center justify-center group-hover:bg-blue-100 transition-colors">
                  <Plug :size="16" />
                </div>
                <div>
                  <div class="font-medium">{{ $t('tools.mcpServer') }}</div>
                  <div class="text-[var(--font-size-xs)] text-gray-400" >{{ $t('tools.mcpServerDesc') }}</div>
                </div>
              </button>
            </div>
          </div>
        </div>

        <button @click="load"
          style="background: var(--btn-primary-bg) !important"
          @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--color-primary)'"
          class="p-2.5 text-white rounded transition-all">
          <RefreshCw :size="16" :class="{ 'animate-spin': loading }" />
        </button>
      </div>
    </div>

    <!-- Stats Cards -->
    <div class="tools-stats-row">
      <div
        class="stats-card bg-white border border-gray-100 rounded-lg p-3  flex items-center gap-4 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)] hover:shadow-md transition-shadow"
        :style="cardBgStyle">
        <div
          class="w-10 h-10  rounded-lg	 bg-indigo-50 text-indigo-600 flex items-center justify-center border border-indigo-100">
          <Wrench :size="24" />
        </div>
          <div class="text-base font text-[var(--text-secondary)]">{{ $t('tools.totalTools') }}</div>
          <div class="text-3xl font-bold text-[var(--text-primary)]">{{ stats.total }}</div>
      </div>
      <div
        class="stats-card bg-white border border-gray-100 rounded-lg p-3 flex items-center gap-4 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)] hover:shadow-md transition-shadow"
        :style="cardBgStyle">
        <div
          class="w-10 h-10 rounded-lg flex items-center justify-center"
          style="background: #ecfdf5; color: #059669; border: 1px solid #d1fae5;">
          <Check :size="24" />
        </div>
          <div class="text-base font text-[var(--text-secondary)]">{{ $t('tools.enabledStatus') }}</div>
          <div class="text-3xl font-bold text-[var(--text-primary)]">{{ stats.active }}</div>
        </div>
      <div
        class="stats-card bg-white border border-gray-100 rounded-lg p-3 flex items-center gap-4 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)] hover:shadow-md transition-shadow"
        :style="cardBgStyle">
        <div
          class="w-10 h-10 rounded-lg	 bg-violet-50 text-violet-600 flex items-center justify-center border border-violet-100">
          <Globe :size="24" />
        </div>
          <div class="text-base font text-[var(--text-secondary)]">{{ $t('tools.coveredTypes') }}</div>
          <div class="text-3xl font-bold text-[var(--text-primary)]">{{ stats.categories }}</div>
      </div>
    </div>

    <!-- Filters Bar -->
    <div class="flex items-center gap-3 mb-4  p-0" >
      <div class="relative flex-1 max-w-md">
        <Search :size="16" class="absolute left-3.5 top-1/2 -translate-y-1/2" style="color: var(--text-secondary);" />
        <input v-model="searchQuery" :placeholder="$t('tools.searchPlaceholder')"
          class="tools-search-input w-full pl-10 py-2 text-base bg-gray-50 border border-[var(--border-color)] rounded focus:bg-white outline-none transition-all placeholder-gray-400"
          :class="searchQuery ? 'pr-9' : 'pr-4'" />
        <SearchClearButton v-if="searchQuery" overlay @clear="searchQuery = ''" />
      </div>

      <div class="h-6 w-px bg-gray-200 mx-1"></div>

      <div class="relative">
        <button @click.stop="showTypeMenu = !showTypeMenu; showStatusMenu = false"
          class="tools-filter-btn px-4 py-2 text-base font rounded hover:bg-gray-50 inline-flex items-center gap-2 transition-colors"
          :style="filterBtnStyle"
          :class="typeFilter !== 'all' ? 'text-[#2B53E5]' : ''">
          {{typeOptions.find(t => t.value === typeFilter)?.label}}
          <ChevronDown :size="14" class="opacity-60" />
        </button>
        <div v-if="showTypeMenu"
          class="tools-filter-menu absolute top-full left-0 mt-2 w-40 rounded-lg shadow-xl z-20 overflow-hidden animate-in fade-in zoom-in-95 duration-150"
          :style="filterMenuStyle">
          <button v-for="opt in typeOptions" :key="opt.value" @click="typeFilter = opt.value; showTypeMenu = false"
            class="w-full text-left px-4 py-2.5 text-base hover:bg-gray-50 transition-colors"
            :style="filterMenuItemStyle"
            :class="typeFilter === opt.value ? 'bg-gray-50' : ''">
            {{ opt.label }}
          </button>
        </div>

      </div>

      <div class="relative">
        <button @click.stop="showStatusMenu = !showStatusMenu; showTypeMenu = false"
          class="tools-filter-btn px-4 py-2 text-base font rounded hover:bg-gray-50 inline-flex items-center gap-2 transition-colors"
          :style="filterBtnStyle"
          :class="statusFilter !== 'all' ? 'text-[#2B53E5]' : ''">
          {{statusOptions.find(s => s.value === statusFilter)?.label}}
          <ChevronDown :size="14" class="opacity-60" />
        </button>
        <div v-if="showStatusMenu"
          class="tools-filter-menu absolute top-full left-0 mt-2 w-40 rounded-lg shadow-xl z-20 overflow-hidden animate-in fade-in zoom-in-95 duration-150"
          :style="filterMenuStyle">
          <button v-for="opt in statusOptions" :key="opt.value"
            @click="statusFilter = opt.value; showStatusMenu = false"
            class="w-full text-left px-4 py-2.5 text-base hover:bg-gray-50 transition-colors"
            :style="filterMenuItemStyle"
            :class="statusFilter === opt.value ? 'bg-gray-50' : ''">
            {{ opt.label }}
          </button>
        </div>
      </div>
      <div class="view-mode-toggle ml-auto flex bg-gray-100 p-1 rounded" :style="toggleStyle">
        <button @click="viewMode = 'card'"
          class="px-3 py-1.5 text-base font rounded transition-all flex items-center gap-1"
          :class="viewMode === 'card' ? 'active-toggle' : 'inactive-toggle'">
          <Box :size="14" /> {{ $t('tools.card') }}
        </button>
        <button @click="viewMode = 'list'"
          class="px-3 py-1.5 text-base font rounded transition-all flex items-center gap-1"
          :class="viewMode === 'list' ? 'active-toggle' : 'inactive-toggle'">
          <List :size="14" /> {{ $t('tools.list') }}
        </button>
      </div>
    </div>

    <!-- Error -->
    <div v-if="error"
      class="mb-4 p-3 bg-red-50 border border-red-200 text-red-700 text-base rounded-lg flex items-center gap-2">
      <span class="flex-1">{{ error }}</span>
      <button @click="error = ''" class="text-red-400 hover:text-red-600">
        <X :size="14" />
      </button>
    </div>

    <div v-if="loading" class="tools-loading-state flex-1 min-h-0 flex flex-col items-center justify-center gap-3 py-24">
      <Loader2 :size="32" class="animate-spin" style="color: var(--text-primary);" />
      <span class="text-sm font-medium" style="color: var(--text-secondary);">{{ $t('common.loading') }}</span>
    </div>

    <!-- Grid View（默认仍为列表；空状态在卡片区域居中显示） -->
    <div v-else-if="viewMode === 'card'" class="tools-grid overflow-y-auto flex-1 min-h-0 pb-4" style="padding-right: 4px;"
      :class="{ 'tools-grid--empty': !filtered.length && !loading }">
      <div v-if="!filtered.length && !loading" class="tools-empty-state">
        <div class="tools-empty-icon">
          <Wrench :size="50" color="var(--text-primary)" :stroke-width="1.5" />
        </div>
        <p class="text-base" style="color: var(--text-primary);">{{ $t('tools.noTools') }}</p>
      </div>
      <div v-for="item in pagedItems" :key="item.id"
        class="content-card bg-white rounded-2xl border border-gray-100 shadow-sm hover:shadow-md hover:border-[#2B53E5]/30 transition-all duration-300 flex flex-col group relative overflow-hidden"
        :style="cardBgStyle">

        <!-- Icon (Top Right) -->
        <div class="absolute top-5 right-5 w-12 h-12 rounded-xl bg-gray-50 flex items-center justify-center text-2xl border border-gray-100 shadow-sm flex-shrink-0 group-hover:scale-110 transition-transform duration-300">
          <img v-if="isSvgIcon(item.tool_type)" :src="typeIcon(item.tool_type)" class="w-6 h-6" />
          <span v-else class="text-2xl">{{ typeIcon(item.tool_type) }}</span>
        </div>

        <!-- Card Body -->
        <div class="p-6 flex-1 flex flex-col">
          <!-- Header -->
          <div class="flex items-start justify-between mb-3">
            <div class="pr-14">
              <div class="flex items-center gap-2 mb-1.5">
                <span
                  class="inline-flex items-center gap-1 text-[var(--font-size-xs)] text-[var(--text-primary)] px-1.5 py-0.5 rounded font-bold uppercase tracking-wide border"
                  :class="typeBadgeClass(item.tool_type)">
                  {{ item.tool_type }}
                </span>
                <span class="relative flex h-2 w-2">
                  <span v-if="item.is_active" class="animate-ping absolute inline-flex min-h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                  <span class="relative inline-flex rounded-full h-2 w-2" :class="item.is_active ? 'bg-emerald-500' : 'bg-gray-300'"></span>
                </span>
              </div>
              <h3 class="text-base font-bold text-[var(--text-primary)] truncate leading-tight" :title="item.display_name || item.name">
                {{ item.display_name || item.name }}
              </h3>
            </div>
          </div>

          <!-- Description -->
          <p class="text-[var(--font-size-base)] text-[var(--text-secondary)] truncate leading-relaxed mb-4">
            {{ item.description || $t('tools.noDescription') }}
          </p>

          <!-- Tags -->
          <div class="flex flex-wrap gap-2 mt-auto">
            <span
              class="inline-flex items-center gap-1 text-[var(--font-size-xs)] px-2 py-1 rounded-full border border-gray-100">
              {{ sourceLabel(item) }}
            </span>
          </div>
        </div>

        <!-- Divider -->
        <div class="h-px bg-gray-50 mx-6"></div>
      </div>
    </div>

    <!-- Table：表头固定，仅表体滚动 -->
    <div v-else
      class="table-card flex-1 min-h-0">
      <div ref="toolsTableHeadRef" class="tools-table-head">
        <table class="tools-table">
          <colgroup>
            <col class="col-name" />
            <col class="col-type" />
            <col class="col-source" />
            <col class="col-desc" />
            <col class="col-status" />
            <col class="col-action" />
          </colgroup>
          <thead>
            <tr>
              <th class="text-left">{{ $t('tools.columnName') }}</th>
              <th class="text-center">{{ $t('tools.columnType') }}</th>
              <th class="text-center">{{ $t('tools.columnSource') }}</th>
              <th class="text-center">{{ $t('tools.columnDesc') }}</th>
              <th class="text-center">{{ $t('tools.columnStatus') }}</th>
              <th class="text-right">{{ $t('tools.columnAction') }}</th>
            </tr>
          </thead>
        </table>
      </div>
      <div ref="toolsTableBodyRef" class="tools-table-body-scroll" @scroll="onToolsTableBodyScroll"
        :class="{ 'tools-table-body-scroll--empty': !filtered.length && !loading }">
        <div v-if="!filtered.length && !loading" class="tools-empty-state">
          <div class="tools-empty-icon">
            <Wrench :size="50" color="var(--text-primary)" :stroke-width="1.5" />
          </div>
          <p class="text-base" style="color: var(--text-primary);">{{ $t('tools.noTools') }}</p>
        </div>
        <table v-else class="tools-table">
          <colgroup>
            <col class="col-name" />
            <col class="col-type" />
            <col class="col-source" />
            <col class="col-desc" />
            <col class="col-status" />
            <col class="col-action" />
          </colgroup>
          <tbody class="tools-table-body">
            <tr v-for="item in pagedItems" :key="item.id" class="group">
              <td>
                <div class="cell-name-title font-bold text-[var(--text-primary)] text-sm mb-0.5" :title="item.display_name || item.name">{{ item.display_name || item.name }}</div>
                <div class="cell-name-sub text-sm text-[var(--text-primary)] font-mono" :title="item.name">{{ item.name }}</div>
              </td>
              <td class="is-center">
                <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-sm font-medium border"
                  :class="typeBadgeClass(item.tool_type)">
                  <img v-if="isSvgIcon(item.tool_type)" :src="typeIcon(item.tool_type)" class="w-3.5 h-3.5 inline-block" /><span v-else class="text-sm">{{ typeIcon(item.tool_type) }}</span> {{ item.tool_type }}
                </span>
              </td>
              <td class="is-center text-sm text-[var(--text-primary)] font-medium">{{ sourceLabel(item) }}</td>
              <td class="is-center text-sm text-[var(--text-primary)] font-medium truncate">{{ item.description || '—' }}</td>
              <td class="is-center">
                <button @click="toggleActive(item)"
                  class="inline-flex items-center gap-1.5 text-sm font-medium transition-colors hover:opacity-80">
                  <component :is="item.is_active ? ToggleRight : ToggleLeft" :size="24"
                    :style="{ color: item.is_active ? (useWhiteIcon() ? '#fff' : '#059669') : 'var(--text-secondary)' }" />
                  <span :style="{ color: item.is_active ? (useWhiteIcon() ? '#fff' : '#059669') : 'var(--text-secondary)' }">
                    {{ item.is_active ? $t('tools.enable') : $t('tools.disable') }}
                  </span>
                </button>
              </td>
              <td class="text-right">
                <div class="text-[var(--text-primary)] font-medium">
                  <button @click="remove(item)" :disabled="item.tool_type === 'MCP'"
                    class="p-2 rounded-lg transition-colors inline-flex text-[var(--text-primary)] font-medium delete-btn"
                    :class="item.tool_type === 'MCP' ? 'text-gray-200 cursor-not-allowed' : 'text-gray-400'"
                    :title="item.tool_type === 'MCP' ? $t('tools.mcpToolTip') : $t('tools.deleteTool')">
                    <Trash2 :size="16" />
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 分页控件 -->
    <div v-if="filtered.length > pageSize"
      class="tools-pagination flex-shrink-0 flex items-center justify-between px-4 py-3 bg-white border border-gray-100 rounded-xl mt-3 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)]"
      :style="cardBgStyle">
      <span class="text-md text-gray-500">{{ $t('tools.showing', { start: pageRange.start, end: pageRange.end, total: pageRange.total }) }}</span>
      <div class="flex items-center gap-1">
        <button @click="currentPage = Math.max(1, currentPage - 1)" :disabled="currentPage <= 1"
          class="px-3 py-1.5 text-md border rounded-lg disabled:opacity-40 disabled:cursor-not-allowed pagination-btn">{{ $t('tools.prevPage') }}</button>
        <template v-for="p in totalPages" :key="p">
          <button v-if="p <= 5 || p === totalPages || Math.abs(p - currentPage) <= 1" @click="currentPage = p"
            class="w-8 h-8 text-md rounded-lg"
            :class="p === currentPage ? 'text-white' : 'hover:bg-[var(--dropdown-hover-color)] '" :style="p === currentPage ? { background: 'var(--btn-primary-bg) !important' } : {}">{{ p }}</button>
          <span v-else-if="p === 6 || p === totalPages - 1" class="text-gray-400 text-md px-1">...</span>
        </template>
        <button @click="currentPage = Math.min(totalPages, currentPage + 1)" :disabled="currentPage >= totalPages"
          class="px-3 py-1.5 text-md border rounded-lg disabled:opacity-40 disabled:cursor-not-allowed pagination-btn">{{ $t('tools.nextPage') }}</button>
      </div>
    </div>

    <!-- Create HTTP Tool Dialog -->
    <Teleport to="body">
      <div v-if="showHttpDialog"
        class="fixed inset-0 z-[100] flex items-center justify-center p-6 transition-all"
        style="background: rgba(15, 23, 42, 0.15); backdrop-filter: blur(0px);">
        <div
          class="tool-form-dialog w-full max-w-lg mx-4 max-h-[86vh] flex flex-col overflow-hidden"
          style="border-radius: 18px; border: 1px solid var(--border-color); box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24); background: var(--theme-dailog-color); color: var(--text-primary);">

          <!-- Header -->
          <div class="flex items-center justify-between px-6 py-5 border-b flex-shrink-0" style="border-color: var(--border-color);">
            <div class="flex items-center gap-3">
              <div
                class="w-10 h-10 bg-gradient-to-br from-emerald-500 to-emerald-600 rounded-lg flex items-center justify-center text-white shadow-lg shadow-emerald-500/20">
                <Globe :size="20" />
              </div>
              <div>
                <h2 class="text-lg font-bold" style="color: var(--text-primary);">{{ $t('tools.createHttpTitle') }}</h2>
                <p class="text-xs" style="color: var(--text-secondary);">{{ $t('tools.createHttpDesc') }}</p>
              </div>
            </div>
            <button @click="showHttpDialog = false"
              class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);" @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'" @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
              <X :size="20" />
            </button>
          </div>

          <!-- Body -->
          <div class="flex-1 overflow-y-auto p-6 space-y-5">
            <!-- Basic Info -->
            <div class="p-5 rounded-lg space-y-4 shadow-sm" style="background: var(--theme-dailog-color); border: 1px solid var(--border-color);">
              <div class="text-base font-bold uppercase tracking-wider mb-2" style="color: var(--text-primary);">{{ $t('tools.basicInfo') }}</div>
              <div class="grid grid-cols-2 gap-4">
                <div class="col-span-2">
                  <label class="block text font-medium mb-1.5" style="color: var(--text-secondary);">{{ $t('tools.toolId') }} <span
                      class="text-red-500">*</span></label>
                  <input v-model="httpForm.name"
                    class="w-full px-4 py-2.5 text border border-transparent rounded-lg focus:border-[var(--color-primary)] focus:ring-4 focus:ring-[var(--color-primary)]/[0.1] outline-none transition-all"
                    style="background: var(--bg-hover); color: var(--text-primary);" placeholder="e.g. search_google" />
                </div>
                <div class="col-span-2">
                  <label class="block text font-medium mb-1.5" style="color: var(--text-secondary);">{{ $t('tools.displayName') }}</label>
                  <input v-model="httpForm.display_name"
                    class="w-full px-4 py-2.5 text border border-transparent rounded-lg focus:border-[var(--color-primary)] focus:ring-4 focus:ring-[var(--color-primary)]/[0.1] outline-none transition-all"
                    style="background: var(--bg-hover); color: var(--text-primary);" :placeholder="$t('tools.displayNamePlaceholder')" />
                </div>
                <div class="col-span-2">
                  <label class="block text font-medium mb-1.5" style="color: var(--text-secondary);">{{ $t('tools.descLabel') }}</label>
                  <input v-model="httpForm.description"
                    class="w-full px-4 py-2.5 text border border-transparent rounded-lg focus:border-[var(--color-primary)] focus:ring-4 focus:ring-[var(--color-primary)]/[0.1] outline-none transition-all"
                    style="background: var(--bg-hover); color: var(--text-primary);" :placeholder="$t('tools.descPlaceholder')" />
                </div>
              </div>
            </div>

            <!-- Request Config -->
            <div class="p-5 rounded-lg space-y-4 shadow-sm" style="background: var(--theme-dailog-color) ; border: 1px solid var(--border-color);">
              <div class="text-base font-bold uppercase tracking-wider mb-2" style="color: var(--text-primary);">{{ $t('tools.requestConfig') }}</div>

              <div class="flex gap-3">
                <div class="w-28 flex-shrink-0">
                  <label class="block text font-medium mb-1.5" style="color: var(--text-secondary);">Method</label>
                  <el-select v-model="httpForm.method" class="method-select" style="width: 100%;">
                    <el-option value="GET" label="GET" />
                    <el-option value="POST" label="POST" />
                    <el-option value="PUT" label="PUT" />
                    <el-option value="DELETE" label="DELETE" />
                  </el-select>
                </div>
                <div class="flex-1">
                  <label class="block text font-medium mb-1.5" style="color: var(--text-secondary);">Endpoint URL <span
                      class="text-red-500">*</span></label>
                  <input v-model="httpForm.url"
                    class="w-full px-4 py-2.5 text font-mono border border-transparent rounded-lg focus:border-[var(--color-primary)] focus:ring-4 focus:ring-[var(--color-primary)]/[0.1] outline-none transition-all"
                    style="background: var(--bg-hover); color: var(--text-primary);" placeholder="https://api.example.com/v1/resource" />
                </div>
              </div>

              <!-- Headers -->
              <div>
                <div class="flex items-center justify-between mb-2">
                  <label class="text font-medium" style="color: var(--text-secondary);">Headers</label>
                  <button @click="addHeader"
                    class="text font-bold flex items-center gap-1 px-2 py-1 rounded transition-colors" style="color: var(--color-primary);"
                    @mouseenter="$event.currentTarget.style.background='var(--dropdown-hover-color)'" 
                    @mouseleave="$event.currentTarget.style.background='transparent'">
                    <Plus :size="12" /> {{ $t('tools.addHeader') }}
                  </button>
                </div>
                <div v-if="!httpForm.headers.length"
                  class="text-center py-4 rounded-lg border border-dashed text" style="background: var(--bg-hover); border-color: var(--border-color); color: var(--text-secondary);">
                  {{ $t('tools.noHeaders') }}
                </div>
                <div v-else class="space-y-2">
                  <div v-for="(h, i) in httpForm.headers" :key="i" class="flex items-center gap-2">
                    <input v-model="h.key"
                      class="w-1/3 px-3 py-2 text font-mono border border-transparent rounded-lg focus:border-[var(--color-primary)] outline-none"
                      style="background: var(--bg-hover); color: var(--text-primary);" placeholder="Key" />
                    <span style="color: var(--text-secondary);">:</span>
                    <input v-model="h.value"
                      class="flex-1 px-3 py-2 text font-mono border border-transparent rounded-lg focus:border-[var(--color-primary)] outline-none"
                      style="background: var(--bg-hover); color: var(--text-primary);" placeholder="Value" />
                    <button @click="removeHeader(i)"
                      class="p-1.5 rounded-lg transition-colors" style="color: var(--text-secondary);" @mouseenter="$event.currentTarget.style.color='var(--color-danger)';$event.currentTarget.style.background='rgba(255,77,79,0.1)'" @mouseleave="$event.currentTarget.style.color='var(--text-secondary)';$event.currentTarget.style.background='transparent'">
                      <X :size="14" />
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Schema -->
            <div class="p-5 rounded-lg space-y-2 shadow-sm flex-1 flex flex-col" style="background: var(--theme-dailog-color); border: 1px solid var(--border-color);">
              <div class="text-base font-bold uppercase tracking-wider mb-1" style="color: var(--text-primary);">{{ $t('tools.paramSchema') }}</div>
              <textarea v-model="httpForm.schema"
                class="code-editor w-full flex-1 min-h-[120px] px-4 py-3 text font-mono rounded-xl outline-none resize-none leading-relaxed"
                style="background: transparent; color: var(--text-primary); border: 1px solid var(--border-color);"
                placeholder='{
  "type": "object",
  "properties": {}
}'
                spellcheck="false" />
            </div>
          </div>

          <!-- Footer -->
          <div class="flex justify-end gap-3 px-6 py-4 border-t flex-shrink-0" style="border-color: var(--border-color);">
            <button @click="showHttpDialog = false"
              style="background: var(--cancel-btn-color); color: var(--btn-secondary-text); border: 1px solid var(--btn-secondary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; transition: all 0.18s ease;"
              @mouseenter="$event.currentTarget.style.background='var(--cancel-btn-hover-color)';$event.currentTarget.style.color='var(--btn-secondary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--cancel-btn-color)';$event.currentTarget.style.color='var(--btn-secondary-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-border)'">{{ $t('common.cancel') }}</button>
            <button @click="createHttpTool" :disabled="!httpForm.name || !httpForm.url || creating"
              style="display: inline-flex;align-items: center; background: var(--btn-primary-bg); color: var(--btn-primary-text); border: 1px solid var(--btn-primary-border); min-width: 106px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; transition: all 0.18s ease;"
              @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)';$event.currentTarget.style.color='var(--btn-primary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-primary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--btn-primary-bg)';$event.currentTarget.style.color='var(--btn-primary-text)';$event.currentTarget.style.borderColor='var(--btn-primary-border)'">
              <Loader2 v-if="creating" :size="16" class="animate-spin" />
              <Check v-else :size="16" />
              <span>{{ $t('tools.createTool') }}</span>
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- Create Skill Dialog -->
    <Teleport to="body">
      <div v-if="showSkillDialog"
        class="fixed inset-0 z-[100] flex items-center justify-center p-6 transition-all"
        style="background: rgba(15, 23, 42, 0.15); backdrop-filter: blur(0px);">
        <div
          class="tool-form-dialog w-full max-w-4xl mx-4 h-[86vh] flex flex-col overflow-hidden"
          style="border-radius: 18px; border: 1px solid var(--border-color); box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24); background: var(--theme-dailog-color); color: var(--text-primary);">

          <!-- Header -->
          <div class="flex items-center justify-between px-6 py-5 border-b flex-shrink-0" style="border-color: var(--border-color);">
            <div class="flex items-center gap-3">
              <div
                class="w-10 h-10 bg-gradient-to-br from-violet-500 to-violet-600 rounded-lg flex items-center justify-center text-white shadow-lg shadow-violet-500/20">
                <Zap :size="20" />
              </div>
              <div>
                <h2 class="text-lg font-bold" style="color: var(--text-primary);">{{ $t('tools.createSkillTitle') }}</h2>
                <p class="text-sm" style="color: var(--text-secondary);">{{ $t('tools.createSkillDesc') }}</p>
              </div>
            </div>
            <button @click="showSkillDialog = false"
              class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);" @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'" @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
              <X :size="20" />
            </button>
          </div>

          <!-- Body: Split Layout -->
          <div class="flex-1 flex overflow-hidden">
            <!-- Left: Info -->
            <div class="w-72 flex-shrink-0 border-r p-6 overflow-y-auto space-y-5" style="border-color: var(--border-color); background: var(--theme-dailog-color);">
              <div>
                <label class="block text-base font-bold mb-1.5" style="color: var(--text-secondary);">{{ $t('tools.skillName') }} <span
                    class="text-red-500">*</span></label>
                <input v-model="skillForm.name"
                  class="w-full px-4 py-2.5 text border border-transparent rounded-lg focus:border-[var(--color-primary)] focus:ring-4 focus:ring-[var(--color-primary)]/[0.1] outline-none transition-all"
                  style="background: var(--bg-hover); color: var(--text-primary);" placeholder="my_skill" />
              </div>

              <div>
                <label class="text-base font-bold mb-1.5" style="color: var(--text-secondary);">{{ $t('tools.category') }}</label>
                <el-select v-model="skillForm.category" style="width: 100%;">
                  <el-option value="general" :label="$t('tools.categoryGeneral')" />
                  <el-option value="search" :label="$t('tools.categorySearch')" />
                  <el-option value="data" :label="$t('tools.categoryData')" />
                  <el-option value="file" :label="$t('tools.categoryFile')" />
                  <el-option value="web" :label="$t('tools.categoryWeb')" />
                </el-select>
              </div>

              <div>
                <label class="block text-base font-bold  mb-1.5" style="color: var(--text-secondary);">{{ $t('tools.version') }}</label>
                <input v-model="skillForm.version"
                  class="w-full px-4 py-2.5 text-sm border border-transparent rounded-lg focus:border-[var(--color-primary)] focus:ring-4 focus:ring-[var(--color-primary)]/[0.1] outline-none transition-all font-mono"
                  style="background: var(--bg-hover); color: var(--text-primary);" placeholder="1.0.0" />
              </div>

              <div>
                <label class="block text-base font-bold mb-1.5" style="color: var(--text-secondary);">{{ $t('tools.descLabel') }}</label>
                <textarea v-model="skillForm.description" rows="4"
                  class="w-full px-4 py-3 text-sm border border-transparent rounded-lg focus:border-[var(--color-primary)] focus:ring-4 focus:ring-[var(--color-primary)]/[0.1] outline-none resize-none transition-all leading-relaxed"
                  style="background: var(--bg-hover); color: var(--text-primary);" :placeholder="$t('tools.skillDescPlaceholder')" />
              </div>
            </div>

            <!-- Right: Code Editor -->
            <div class="flex-1 flex flex-col min-w-0 " style="background: var(--bg-hover); ">
              <div class="flex items-center justify-between px-4 py-2 bg-[var(--bg-hover)] border-b border-[#333]">
                <div class="flex items-center gap-2 text-xs text-[var(--text-primary)]">
                  <div class="w-2.5 h-2.5 rounded-full bg-yellow-500"></div>
                  <span>main.py</span>
                </div>
                <div class="text-[var(--font-size-xs)] text-[var(--text-primary)]">Python 3.10</div>
              </div>
              <textarea v-model="skillForm.code"
                class="code-editor flex-1 w-full p-4 text-xs font-mono bg-[var(--bg-hover)] text-[var(--text-primary)] outline-none resize-none leading-loose border-none selection:bg-[#264F78]"
                spellcheck="false" />
            </div>
          </div>

          <!-- Footer -->
          <div class="flex justify-end gap-3 px-6 py-4 border-t flex-shrink-0" style="border-color: var(--border-color);">
            <button @click="showSkillDialog = false"
              style="background: var(--cancel-btn-color); color: var(--btn-secondary-text); border: 1px solid var(--btn-secondary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; transition: all 0.18s ease;"
              @mouseenter="$event.currentTarget.style.background='var(--cancel-btn-hover-color)';$event.currentTarget.style.color='var(--btn-secondary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--cancel-btn-color)';$event.currentTarget.style.color='var(--btn-secondary-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-border)'">{{ $t('common.cancel') }}</button>
            <button @click="createSkillTool" :disabled="!skillForm.name || creating"
              :style="{ display: 'inline-flex', alignItems: 'center', background: 'var(--btn-primary-bg)', color: 'var(--btn-primary-text)', border: '1px solid var(--btn-primary-border)', minWidth: '100px', width: 'auto', height: '36px', padding: '0 16px', borderRadius: '4px', cursor: (!skillForm.name || creating) ? 'not-allowed' : 'pointer', fontSize: 'var(--font-size-base)', fontWeight: '600', transition: 'all 0.18s ease' }"
              @mouseenter="if (!(!skillForm.name || creating)) { $event.currentTarget.style.background='var(--btn-primary-hover-bg)';$event.currentTarget.style.color='var(--btn-primary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-primary-hover-border)' }"
              @mouseleave="if (!(!skillForm.name || creating)) { $event.currentTarget.style.background='var(--btn-primary-bg)';$event.currentTarget.style.color='var(--btn-primary-text)';$event.currentTarget.style.borderColor='var(--btn-primary-border)' }">
              <Loader2 v-if="creating" :size="16" class="animate-spin" />
              <Check v-else :size="16" />
              <span>{{ $t('tools.createSkill') }}</span>
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.tools-page {
  box-sizing: border-box;
  height: 100%;
  min-height: 0;
}

.tools-stats-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 20px;
  margin-bottom: 12px;
}

.tools-stats-row .stats-card {
  min-width: 0;
}

.tools-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
  align-content: start;
  align-items: start;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;
}

.tools-grid--empty {
  display: flex;
  grid-template-columns: none;
  align-items: center;
  justify-content: center;
}
.tools-grid::-webkit-scrollbar { width: 6px; }
.tools-grid::-webkit-scrollbar-track { background: transparent; }
.tools-grid::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, var(--webkit-scrollbar-thumb, rgba(0, 0, 0, 0.15)));
  border-radius: 3px;
}
.tools-grid::-webkit-scrollbar-thumb:hover {
  background: var(--scrollbar-thumb-hover, rgba(0, 0, 0, 0.18));
}

.badge-mcp {
  background: transparent;
  color: var(--text-primary);
  border-color: var(--border-color);
}

.badge-mcp .icon {
  color: #7C3AED;
}

.badge-http {
  background: transparent;
  color: var(--text-primary);
  border-color: var(--border-color);
}

.badge-http .icon {
  color: #10B981;
}

.badge-skill {
  background: transparent;
  color: var(--text-primary);
  border-color: var(--border-color);
}

/* 视图切换按钮 */
.active-toggle {
  background: #fff;
  color: var(--text-primary);
  box-shadow: 0 1px 2px rgba(0,0,0,0.1);
}
.inactive-toggle {
  color: var(--text-secondary);
}
.inactive-toggle:hover {
  color: var(--text-primary);
}

/* 情景主题（除雾灰调） */
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .active-toggle {
  background: rgba(255,255,255,0.15);
  color: #fff;
}
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .inactive-toggle {
  color: rgba(255,255,255,0.6);
}
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .inactive-toggle:hover {
  color: #fff;
}
.dark .active-toggle {
  background: rgba(255,255,255,0.15);
  color: #fff;
}
.dark .inactive-toggle {
  color: rgba(255,255,255,0.6);
}
.dark .inactive-toggle:hover {
  color: #fff;
}

.badge-skill .icon {
  color: #F59E0B;
}
</style>

<style>
/* ── Title / Subtitle ── */
.tools-title { color: #1f2937; }
.tools-subtitle { color: #6b7280; }
.dark .tools-title { color: #ffffff !important; }
.dark .tools-subtitle { color: #cbd5e0 !important; }

.tool-form-dialog input {
  border-radius: 4px;   /* 改这个值，越大越圆 */
}
.tool-form-dialog .el-select__wrapper {
  border-radius: 4px;   /* 下拉框同步 */
}
/* ── Stats cards ── */
.dark .stats-card {
  background: rgba(255,255,255,0.15) !important;
  border-color: rgba(255,255,255,0.12) !important;
}
.dark .stats-card .text-2xl,
.dark .stats-card .text-xs {
  color: #ffffff !important;
}
.dark .stats-card .text-\[\#1D2129\],
.dark .stats-card .text-\[\#86909C\] {
  color: #ffffff !important;
}

/* ── Content cards (grid view + table) ── */
.dark .content-card {
  background: rgba(255,255,255,0.15) !important;
  border-color: rgba(255,255,255,0.12) !important;
}
.dark .content-card .text-\[\#1D2129\],
.dark .content-card .text-sm,
.dark .content-card .text-xs {
  color: #ffffff !important;
}
.dark .content-card .text-\[\#86909C\] {
  color: #cbd5e0 !important;
}

/* ── Filter bar ── */
.dark .items-center.gap-3.mb-4.bg-white {
  background: rgba(255,255,255,0.06) !important;
  border-color: rgba(255,255,255,0.1) !important;
}
.dark .items-center.gap-3.mb-4 input {
  background: transparent;
  color: #ffffff;
}
/* 搜索框聚焦时边框颜色不变 */
.tools-search-input:focus,
.tools-search-input:focus-visible {
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  outline: none !important;
}
.light [data-theme-id="scene-graphite-cyan"] .items-center.gap-3.mb-4 input,
.light [data-theme-id="simple-light"] .items-center.gap-3.mb-4 input {
  background: transparent;
}
/* 情景主题（除雾灰调）：白色文字，搜索框透明，表头透明 */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .items-center.gap-3.mb-4 input {
  background: transparent;
  color: #fff;
}
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .items-center.gap-3.mb-4 input::placeholder {
  color: rgba(255,255,255,0.6);
}
/* 下拉触发按钮 hover + 文字 */
.light [data-theme-id]:not([data-theme-id="scene-graphite-cyan"]) .items-center.gap-3.mb-4 button:not(.skills-search-clear) {
  color: #fff;
}
.light [data-theme-id]:not([data-theme-id="scene-graphite-cyan"]) .items-center.gap-3.mb-4 button:not(.skills-search-clear):hover {
  background: rgba(255,255,255,0.10) !important;
}
.dark .items-center.gap-3.mb-4 input::placeholder {
  color: rgba(255,255,255,0.6);
}
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-title,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-subtitle,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .stats-card .text-2xl,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .stats-card .text-xs,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .content-card .text-sm,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .content-card .text-xs { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-table thead th {
  background: var(--table-header-bg-color) !important;
}
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .stats-card,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .content-card,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .rounded-lg.bg-white { border-color: rgba(255,255,255,0.10) !important; }
.dark .items-center.gap-3.mb-6 > .relative > button:hover {
  background: transparent !important;
}

/* ── View mode toggle ── */
/* 浅色主题：当前选中文本/切换按钮 → 黑色 */
.light [data-theme-id="simple-light"] .items-center.gap-3.mb-4 button:not(.skills-search-clear),
.light [data-theme-id="simple-light"] .view-mode-toggle button { color: #1f2937 !important; }

.dark .view-mode-toggle {
  background: rgba(255,255,255,0.06);
}
.dark .view-mode-toggle button.bg-white {
  background: rgba(255,255,255,0.15) !important;
  color: #ffffff !important;
}
.dark .view-mode-toggle .text-gray-500 {
  color: #cbd5e0;
}

/* ── Dropdown menus ── */
/* 浅色主题：纯白背景，黑色文字，hover 黑色5% */
html.light[data-theme-id="simple-light"] .tools-create-menu,
html.light[data-theme-id="simple-light"] .tools-filter-menu,
html.light[data-theme-id="scene-graphite-cyan"] .tools-create-menu,
html.light[data-theme-id="scene-graphite-cyan"] .tools-filter-menu {
  background: #fff !important;
  border-color: #e5e7eb !important;
}
html.light[data-theme-id="simple-light"] .tools-create-menu button,
html.light[data-theme-id="simple-light"] .tools-filter-menu button,
html.light[data-theme-id="scene-graphite-cyan"] .tools-create-menu button,
html.light[data-theme-id="scene-graphite-cyan"] .tools-filter-menu button { color: #1f2937 !important; }
html.light[data-theme-id="simple-light"] .tools-create-menu button:hover,
html.light[data-theme-id="simple-light"] .tools-filter-menu button:hover,
html.light[data-theme-id="scene-graphite-cyan"] .tools-create-menu button:hover,
html.light[data-theme-id="scene-graphite-cyan"] .tools-filter-menu button:hover {
  background: rgba(0,0,0,0.05) !important;
}

/* 情景主题（除雾灰调）：纯色背景，不带透明度 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu.bg-white,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-filter-menu.bg-white {
  background: #ffffff !important;
  border-color: var(--border-color) !important;
  color: #000!important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu button:hover,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-filter-menu button:hover {
  background: var(--bg-hover) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu .text-gray-400,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-filter-menu .text-gray-400 {
  color: var(--text-secondary) !important;
}

/* 暗色主题：纯黑背景，白色文字，hover 白色10% */
.dark .tools-create-menu,
.dark .tools-filter-menu {
  background: #000000 !important;
  border-color: rgba(255,255,255,0.12) !important;
}
.dark .tools-create-menu button,
.dark .tools-filter-menu button { color: #fff !important; }
.dark .tools-create-menu button:hover,
.dark .tools-filter-menu button:hover {
  background: rgba(255,255,255,0.10) !important;
}
.dark .tools-create-menu .text-gray-400,
.dark .tools-filter-menu .text-gray-400 {
  color: #8896a7 !important;
}
.dark .tools-create-menu .text-\[\#1D2129\],
.dark .tools-filter-menu .text-\[\#1D2129\] {
  color: #fff !important;
}

/* 新建API下拉菜单固定为白底黑字，不受主题影响 */
html body .tools-create-menu.tools-create-menu {
  background: #fff !important;
  border-color: #e5e7eb !important;
}

html body .tools-create-menu.tools-create-menu button:hover {
  background: rgba(0,0,0,0.05) !important;
}

/* 新建API下拉菜单图标背景：浅色主题和雾灰调主题保持原色，其它主题设为白色 */
html.dark .tools-create-menu button .create-menu-icon,
html.dark .tools-create-menu button:hover .create-menu-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu button .create-menu-icon,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu button:hover .create-menu-icon {
  background: #ffffff !important;
}

/* 情景主题下保留新建菜单彩色图标（避免被全局 emerald→白 覆盖成白底白字） */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu .create-menu-icon.text-emerald-600,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu .create-menu-icon.text-emerald-600 svg {
  color: #059669 !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu .create-menu-icon.text-violet-600,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu .create-menu-icon.text-violet-600 svg {
  color: #7c3aed !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu .create-menu-icon.text-blue-600,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-create-menu .create-menu-icon.text-blue-600 svg {
  color: #2563eb !important;
}

/* ── Table ── */
.dark .rounded-2xl.border.border-gray-100 {
  border-color: rgba(255,255,255,0.1) !important;
}
.dark .bg-\[\#F7F8FA\] {
  background: rgba(255,255,255,0.06) !important;
  border-color: rgba(255,255,255,0.1) !important;
}
.dark tr.hover\:bg-gray-50\/80:hover {
  background: rgba(255,255,255,0.04) !important;
}

/* ── Pagination ── */
.dark .rounded-xl.bg-white:not(.tools-create-menu):not(.tools-filter-menu) {
  background: rgba(255,255,255,0.06) !important;
  border-color: rgba(255,255,255,0.1) !important;
}

/* ── Table/card misc text in dark theme ── */
.dark .content-card .text-\[\#4E5969\] {
  color: #cbd5e0 !important;
}

/* ── Badges in dark theme ── */
.dark .badge-mcp,
.dark .badge-http,
.dark .badge-skill {
  background: rgba(255,255,255,0.1) !important;
  color: var(--text-primary) !important;
  border-color: rgba(255,255,255,0.15) !important;
}

/* 统计卡片 / 内容卡片 / 表格 背景 */
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .stats-card,
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .content-card { background: rgba(255, 255, 255, 0.10) !important; }
html.light [data-theme-id="simple-light"] .stats-card,
html.light [data-theme-id="simple-light"] .content-card { background: #ffffff !important; }
html.light [data-theme-id="scene-graphite-cyan"] .stats-card,
html.light [data-theme-id="scene-graphite-cyan"] .content-card { background: rgba(0, 0, 0, 0.07) !important; }
/* 情景主题（除雾灰调）：卡片悬停 */
html.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .content-card:hover { background: rgba(255, 255, 255, 0.14) !important; }
.bg-red-50 { background: rgba(255, 255, 255, 0.30) !important; }

/* 浅色主题下弹窗背景设为白色 */
html.light [data-theme-id="simple-light"] .tool-form-dialog {
  background: #ffffff !important;
}

/* HTTP / Skill 弹窗输入框、下拉框背景与边框和设置页一致 */
.tool-form-dialog input,
.tool-form-dialog select,
.tool-form-dialog textarea:not(.code-editor) {
  background: transparent !important;
  border-color: var(--border-color) !important;
}
.tool-form-dialog input:focus,
.tool-form-dialog select:focus,
.tool-form-dialog textarea:not(.code-editor):focus {
  box-shadow: none !important;
  outline: none;
}

/* 表格：表头固定，仅表体区域滚动 */
.table-card {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: transparent;
}

.tools-table-head {
  flex-shrink: 0;
  background: var(--table-header-bg-color);
  overflow: hidden;
  padding-right: 6px;
  box-sizing: border-box;
}

.tools-table-body-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;
  padding-bottom: 2px;
}

.tools-table-body-scroll--empty {
  display: flex;
  align-items: center;
  justify-content: center;
}

.tools-loading-state {
  background: transparent !important;
}

.tools-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  width: 100%;
  min-height: 100%;
  padding: 24px;
  text-align: center;
  color: var(--text-primary);
  box-sizing: border-box;
}

.tools-empty-icon {
  width: 60px;
  height: 60px;
  border-radius: 9999px;
  background: rgba(148, 163, 184, 0.12);
  display: flex;
  align-items: center;
  justify-content: center;
}

.tools-table-body-scroll::-webkit-scrollbar { width: 6px; height: 6px; }
.tools-table-body-scroll::-webkit-scrollbar-track { background: transparent; }
.tools-table-body-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, var(--webkit-scrollbar-thumb, rgba(0, 0, 0, 0.15)));
  border-radius: 3px;
}
.tools-table-body-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--scrollbar-thumb-hover, rgba(0, 0, 0, 0.18));
}

.tools-table {
  width: 100%;
  min-width: 720px;
  table-layout: fixed;
  border-collapse: collapse;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  background: transparent;
}

.tools-table .col-name { width: 16%; }
.tools-table .col-type { width: 13%; }
.tools-table .col-source { width: 13%; }
.tools-table .col-desc { width: 30%; }
.tools-table .col-status { width: 16%; }
.tools-table .col-action { width: 12%; }

.tools-table thead th {
  background: var(--table-header-bg-color) !important;
  font-weight: 600;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  padding: 10px 16px;
  text-align: left;
  border: none;
  white-space: nowrap;
}

.tools-table thead th.text-center { text-align: center; }
.tools-table thead th.text-right { text-align: right; }

.tools-table tbody td {
  padding: 12px 16px;
  border: none;
  color: var(--text-primary);
  background-color: transparent;
  vertical-align: middle;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tools-table tbody td.is-center {
  text-align: center;
}

.tools-table tbody td.is-center > * {
  margin-left: auto;
  margin-right: auto;
}

.tools-table .cell-name-title,
.tools-table .cell-name-sub {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tools-table tbody tr:nth-child(even) td {
  background-color: var(--card-background-tmd);
}

.tools-table tbody tr:hover td {
  background-color: var(--dropdown-hover-color);
}

.tools-table tbody tr:nth-child(even):hover td {
  background-color: var(--dropdown-hover-color);
}

/* 表格横线颜色（兼容旧选择器） */
.tools-table-body tr {
  border-bottom: none;
}
.tools-table-body tr:last-child {
  border-bottom: none;
}

/* 情景主题下（除雾灰调外）状态列图标和文字颜色为白色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-table-body .text-emerald-500,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-table-body .text-emerald-600,
html.light[data-theme-id="custom-scene"] .tools-table-body .text-emerald-500,
html.light[data-theme-id="custom-scene"] .tools-table-body .text-emerald-600 {
  color: #ffffff !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-table-body .text-gray-300,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-table-body .text-gray-400,
html.light[data-theme-id="custom-scene"] .tools-table-body .text-gray-300,
html.light[data-theme-id="custom-scene"] .tools-table-body .text-gray-400 {
  color: rgba(255, 255, 255, 0.7) !important;
}

/* 黑暗主题和情景主题（除雾灰调外）类型列 SVG 图标颜色为白色 */
.dark .tools-table-body td img[src$=".svg"],
.dark .content-card img[src$=".svg"],
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-table-body td img[src$=".svg"],
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .content-card img[src$=".svg"] {
  filter: brightness(0) invert(1);
}

/* 除浅色主题外，删除按钮颜色适配 */
html.dark .delete-btn,
html.light[data-theme-id^="scene-"] .delete-btn { color: rgba(255,255,255,0.7) !important; }
html.light[data-theme-id="scene-graphite-cyan"] .delete-btn,
html.light[data-theme-id="simple-light"] .delete-btn { color: #1f2937 !important; }

html.dark .delete-btn:not(:disabled):hover,
html.light[data-theme-id^="scene-"] .delete-btn:not(:disabled):hover,
html.light[data-theme-id="scene-graphite-cyan"] .delete-btn:not(:disabled):hover {
  background: rgba(0, 0, 0, 0.05) !important;
  color: #ef4444 !important;
}

/* 浅色主题下删除按钮保持原有样式 */
html.light[data-theme-id="simple-light"] .delete-btn:not(:disabled):hover {
  background: #fef2f2 !important;
  color: #ef4444 !important;
}
/* Method下拉框和Skill分类下拉框高度与输入框一致 */
.tool-form-dialog .method-select .el-select__wrapper,
.tool-form-dialog .el-select .el-select__wrapper {
  padding: 10px 16px !important;
  height: 40px !important;
  min-height: unset !important;
  box-sizing: border-box !important;
  background: transparent !important;
}
.tool-form-dialog input {
  height: 40px;
  box-sizing: border-box;
}

/* 筛选按钮悬浮背景 */
.dark .tools-filter-btn:hover { background: rgba(255,255,255,0.08) !important; }

.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-filter-btn:hover { background: rgba(255,255,255,0.10) !important; }

/* 分页按钮样式 */
.pagination-btn {
  border-color: var(--border-color) !important;
}
.pagination-btn:not(:disabled):hover {
  background: rgba(0, 0, 0, 0.05) !important;
}
.light [data-theme-id="simple-light"] .tools-filter-btn:hover,
.light [data-theme-id="scene-graphite-cyan"] .tools-filter-btn:hover { background: rgba(0,0,0,0.10) !important; }

/* 删除确认弹窗：取消按钮与 .theme-action--secondary 一致 */
html body .el-message-box {
  .el-button.el-button--default {
    min-width: 76px !important;
    height: 36px !important;
    padding: 0 16px !important;
    border-radius: 4px !important;
    font-size: var(--font-size-base) !important;
    font-weight: 600 !important;
    background: var(--cancel-btn-color) !important;
    color: var(--btn-secondary-text) !important;
    border: 1px solid var(--btn-secondary-border) !important;
    box-shadow: none !important;

    &:hover,
    &:focus,
    &:active {
      background: var(--cancel-btn-hover-color) !important;
      color: var(--btn-secondary-hover-text) !important;
      border-color: var(--btn-secondary-hover-border) !important;
      box-shadow: none !important;
    }
  }

  .el-button.el-button--primary {
    min-width: 76px !important;
    height: 36px !important;
    padding: 0 16px !important;
    border-radius: 4px !important;
    font-size: var(--font-size-base) !important;
    font-weight: 600 !important;
  }
}

</style>
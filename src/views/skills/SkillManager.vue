<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import {
  Plus, Trash2, Search, Upload, RefreshCw, Power, X, Check,
  Loader2, FileText, Zap, ToggleLeft, ToggleRight, ChevronDown, Edit3, Eye, ChevronRight, Download
} from 'lucide-vue-next'
import { skillsApi } from '../mcp/adminApi.js'
import AiDescriptionButton from '../mcp/AiDescriptionButton.vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useI18n } from 'vue-i18n'
import MarkdownIt from 'markdown-it'
import sdIcon from '@/assets/alltool/sd.svg'
import wlIcon from '@/assets/skills/wl.svg'
import tyIcon from '@/assets/skills/ty.svg'
import yjIcon from '@/assets/skills/yj.svg'
import yfIcon from '@/assets/skills/yf.svg'
import wbIcon from '@/assets/skills/wb.svg'
import analysisIcon from '@/assets/skills/fx.svg'
import sjIcon from '@/assets/skills/sj.svg'
import tpIcon from '@/assets/skills/tp.svg'
import wjIcon from '@/assets/skills/wj.svg'
import tyIconWhite from '@/assets/skills/ty_white.svg'
import yjIconWhite from '@/assets/skills/yj_white.svg'
import yfIconWhite from '@/assets/skills/yf_white.svg'
import wbIconWhite from '@/assets/skills/wb_white.svg'
import analysisIconWhite from '@/assets/skills/fx_white.svg'
import sjIconWhite from '@/assets/skills/sj_white.svg'
import tpIconWhite from '@/assets/skills/tp_white.svg'
import wlIconWhite from '@/assets/skills/wl_white.svg'
import wjIconWhite from '@/assets/skills/wj_white.svg'
import runningIcon from '@/assets/skills/running.svg'
import typeIcon from '@/assets/skills/type.svg'
import scoreIcon from '@/assets/skills/score.svg'
import { useThemeStore } from '@/stores/theme'
import { useDialog } from '@/composables/useDialog'
import SearchClearButton from '@/components/common/SearchClearButton.vue'

const { t } = useI18n()
const { confirm: confirmDialog } = useDialog()
const themeStore = useThemeStore()

const md = new MarkdownIt({ html: true, linkify: true, breaks: true })

const cardBgStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-')) return { 'background-color': 'rgba(255,255,255,0.10)' }
  if (id === 'simple-light' || id === 'scene-graphite-cyan') return { 'background-color': 'rgba(0,0,0,0.07)' }
  if (id.startsWith('scene-')) return { 'background-color': 'rgba(255,255,255,0.05)' }
  return {}
})

const searchInputStyle = computed(() => ({
  background: 'transparent',
  border: '1px solid var(--border-color)',
  color: 'var(--text-primary)',
}))

const filterBtnStyle = computed(() => ({
  background: 'transparent',
  border: '1px solid var(--border-color)',
  color: 'var(--text-primary)',
}))

const darkTextStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-')) return { color: '#e5e7eb' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan') return { color: '#e5e7eb' }
  return {}
})

const skillsTableHeadRef = ref(null)
const skillsTableBodyRef = ref(null)

function onSkillsTableBodyScroll() {
  const head = skillsTableHeadRef.value
  const body = skillsTableBodyRef.value
  if (head && body) head.scrollLeft = body.scrollLeft
}

const items = ref([])
const loading = ref(true)
const refreshing = ref(false)
const error = ref('')
const searchQuery = ref('')
const debouncedSearchQuery = ref('')
let searchTimer = null
function setSearchQuery(q) {
  searchQuery.value = q
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    debouncedSearchQuery.value = q.trim().toLowerCase()
  }, 250)
}
const categoryFilter = ref('all')
const sortKey = ref('name_asc')
const showCategoryMenu = ref(false)
const showSortMenu = ref(false)
const showCreateDialog = ref(false)
const showDetailDialog = ref(false)
const selectedItem = ref(null)
const fullSkill = ref(null)       // 完整 skill 数据（get 拉取）
const loadingDetail = ref(false)
const detailLang = ref('zh')
const detailEditing = ref(false)
const detailEditLang = ref('zh')
const detailSaving = ref(false)
const detailError = ref('')
const rebuilding = ref(false)
const selectedIds = ref(new Set())
const batchLoading = ref(false)
const pendingIds = ref(new Set())

// Create form
const form = ref({
  name: '', description: '', description_en: '', category: 'general',
  author: '', version: '1.0.0', content_zh: '', content_en: '', folder: '',
  // 数据型声明：产出是天气/股价这类实时数据 → 拿不到证据时系统拒绝作答
  live_data: false,
  // 表单里用逗号分隔的字符串，提交前 parseToolList 转数组
  requires_tools: ''
})

/** 「必需工具」输入 → 数组。空串表示"没声明"，不是"声明了一个空的" */
function parseToolList(v) {
  return String(v || '').split(',').map(s => s.trim()).filter(Boolean)
}

/** 数组 → 输入框文本（回显用） */
function joinToolList(v) {
  return Array.isArray(v) ? v.join(', ') : ''
}
const createMode = ref('manual')
const createLang = ref('zh')
const editingSkill = ref(null)
const zipFile = ref(null)
const zipFileName = ref('')
const uploading = ref(false)
const uploadMsg = ref('')
const uploadStatus = ref(null)
const githubUrl = ref('')
const githubSubdir = ref('skills')
const githubRef = ref('main')
const githubOverwrite = ref(false)
const installLoading = ref(false)

const categories = ['general', 'research', 'development', 'text', 'analysis', 'data', 'image', 'web', 'file']

function catLabel(cat) {
  const key = 'skills.cat_' + cat
  const translated = t(key)
  return translated === key ? cat : translated
}

function skillLabel(lang, key) {
  return t(`skills.labels.${lang}.${key}`)
}

function popupCategoryLabel(lang, cat) {
  const key = `skills.labels.${lang}.cat_${cat}`
  const translated = t(key)
  return translated === key ? cat : translated
}

const filtered = computed(() => {
  let result = items.value
  if (categoryFilter.value !== 'all') {
    result = result.filter(i => i.category === categoryFilter.value)
  }
  if (!debouncedSearchQuery.value) return result
  const q = debouncedSearchQuery.value
  return result.filter(i =>
    i.name?.toLowerCase().includes(q) || i.description?.toLowerCase().includes(q) || i.description_en?.toLowerCase().includes(q) || i.category?.toLowerCase().includes(q) || (i.tags || []).some(t => String(t).toLowerCase().includes(q))
  )
})

const sorted = computed(() => {
  const list = [...filtered.value]
  if (sortKey.value === 'usage_desc') {
    return list.sort((a, b) => (b.usage_count || 0) - (a.usage_count || 0))
  }
  if (sortKey.value === 'created_desc') {
    return list.sort((a, b) => createdAtValue(b) - createdAtValue(a))
  }
  // default: name_asc
  return list.sort((a, b) => String(a.name || '').localeCompare(String(b.name || '')))
})

// 分页
const currentPage = ref(1)
const pageSize = ref(15)
const totalPages = computed(() => Math.max(1, Math.ceil(sorted.value.length / pageSize.value)))
const pagedItems = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  return sorted.value.slice(start, start + pageSize.value)
})
const pageRange = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value + 1
  const end = Math.min(currentPage.value * pageSize.value, sorted.value.length)
  return { start, end, total: sorted.value.length }
})

// 筛选条件变化 → 页码回第 1 页
watch([debouncedSearchQuery, categoryFilter, sortKey], () => {
  currentPage.value = 1
})
// 页码上界夹紧（删除最后一页最后一条时回退）
watch(totalPages, (tp) => {
  if (currentPage.value > tp) currentPage.value = tp
})
// 数据变更导致当前分类无数据时自动回到全部分类
watch(items, () => {
  if (categoryFilter.value !== 'all' && !items.value.some(i => i.category === categoryFilter.value)) {
    categoryFilter.value = 'all'
  }
})

// 带数量的分类列表
const categoriesWithCount = computed(() =>
  categories.map(cat => ({
    value: cat,
    label: catLabel(cat),
    count: items.value.filter(i => i.category === cat).length,
  }))
)

const stats = computed(() => {
  const total = items.value.length
  const enabled = items.value.filter(i => i.enabled).length
  const cats = new Set(items.value.map(i => i.category).filter(Boolean)).size
  const now = Date.now()
  const weekMs = 7 * 24 * 60 * 60 * 1000
  const recentUsed = items.value.filter(i => {
    if (!i.updated_at && !i.created_at) return false
    const t = Date.parse(i.updated_at || i.created_at || '')
    return !Number.isNaN(t) && now - t < weekMs
  }).length
  return { total, enabled, categories: cats, recentUsed }
})

const categoryIcons = {
  general: tyIcon,
  research: yjIcon,
  development: yfIcon,
  text: wbIcon,
  analysis: analysisIcon,
  data: sjIcon,
  image: tpIcon,
  web: wlIcon,
  file: wjIcon,
}

const categoryIconsWhite = {
  general: tyIconWhite,
  research: yjIconWhite,
  development: yfIconWhite,
  text: wbIconWhite,
  analysis: analysisIconWhite,
  data: sjIconWhite,
  image: tpIconWhite,
  web: wlIconWhite,
  file: wjIconWhite,
}

const svgCategories = new Set(['general', 'research', 'development', 'text', 'analysis', 'data', 'image', 'web', 'file'])

function isSvgIcon(val) {
  return typeof val === 'object' || (typeof val === 'string' && val.endsWith('.svg'))
}

function useWhiteIcon() {
  const id = themeStore.draftPreset.id
  return id === 'simple-dark' || id.startsWith('dark-') || (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
}

function categoryIcon(category) {
  const icons = useWhiteIcon() ? categoryIconsWhite : categoryIcons
  return icons[category] || '🧩'
}

const sortOptions = computed(() => [
  { value: 'name_asc', label: t('skills.sortNameAsc') },
  { value: 'usage_desc', label: t('skills.sortSuccessDesc') },
  { value: 'created_desc', label: t('skills.sortNewest') },
])

const sortLabel = computed(() => {
  return sortOptions.value.find(o => o.value === sortKey.value)?.label || t('skills.sortNameAsc')
})

const categoryLabel = computed(() => {
  return categoryFilter.value === 'all' ? t('skills.allCategories') : catLabel(categoryFilter.value)
})

function categoryBadgeClass(category) {
  if (category === 'data') return 'bg-purple-50 text-purple-700'
  if (category === 'web') return 'bg-sky-50 text-sky-700'
  if (category === 'analysis') return 'bg-cyan-50 text-cyan-700'
  if (category === 'development') return 'bg-indigo-50 text-indigo-700'
  if (category === 'research') return 'bg-violet-50 text-violet-700'
  if (category === 'file') return 'bg-amber-50 text-amber-700'
  return 'bg-indigo-50 text-indigo-700'
}

function iconBgClass(category) {
  if (category === 'data') return 'bg-gradient-to-br from-purple-50 to-purple-100 text-purple-700'
  if (category === 'web') return 'bg-gradient-to-br from-sky-50 to-sky-100 text-sky-700'
  if (category === 'analysis') return 'bg-gradient-to-br from-cyan-50 to-cyan-100 text-cyan-700'
  if (category === 'development') return 'bg-gradient-to-br from-indigo-50 to-indigo-100 text-indigo-700'
  if (category === 'research') return 'bg-gradient-to-br from-violet-50 to-violet-100 text-violet-700'
  if (category === 'file') return 'bg-gradient-to-br from-amber-50 to-amber-100 text-amber-700'
  return 'bg-gradient-to-br from-indigo-50 to-indigo-100 text-indigo-700'
}

function createdAtValue(item) {
  const raw = item.created_at || item.createdAt || item.updated_at || item.updatedAt || 0
  const time = new Date(raw).getTime()
  return Number.isNaN(time) ? 0 : time
}

async function load(opts) {
  const silent = opts?.silent
  if (!silent) loading.value = true
  else refreshing.value = true
  error.value = ''
  try {
    const res = await skillsApi.list()
    items.value = res.items || res || []
  } catch (e) { error.value = e.message }
  finally {
    if (!silent) loading.value = false
    else refreshing.value = false
  }
}

async function toggle(item, skipReload = false) {
  const id = item.name
  if (pendingIds.value.has(id)) return
  pendingIds.value = new Set([...pendingIds.value, id])

  const newEnabled = !item.enabled
  const previousEnabled = item.enabled
  // 乐观更新：立即翻转 UI
  const idx = items.value.findIndex(i => i.name === id)
  if (idx >= 0) items.value[idx] = { ...items.value[idx], enabled: newEnabled }

  try {
    await skillsApi.toggle(id, newEnabled)
    if (!skipReload) load({ silent: true })
  } catch (e) {
    // 回滚
    if (idx >= 0) items.value[idx] = { ...items.value[idx], enabled: previousEnabled }
    ElMessage.error(e.message || t('skills.operationFailed'))
  } finally {
    const next = new Set(pendingIds.value)
    next.delete(id)
    pendingIds.value = next
  }
}

async function remove(item) {
  try {
    await ElMessageBox.confirm(
      t('skills.confirmDeleteSkill', { name: item.name }),
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
    await skillsApi.delete(item.name)
    items.value = items.value.filter(i => i.name !== item.name)
  } catch (e) { error.value = e.message }
}

function openCreateDialog(mode, skill) {
  if (mode === 'github') {
    createMode.value = 'github'
    githubUrl.value = ''
    githubSubdir.value = 'skills'
    githubRef.value = 'main'
    githubOverwrite.value = false
  } else {
    createMode.value = 'manual'
    editingSkill.value = skill || null
    if (skill) {
      form.value = {
        name: skill.name || '',
        description: skill.description || '',
        description_en: skill.description_en || '',
        category: skill.category || 'general',
        author: skill.author || '',
        version: skill.version || '1.0.0',
        content_zh: skill.content_zh || '',
        content_en: skill.content_en || '',
        folder: '',
        live_data: !!skill.live_data,
        requires_tools: joinToolList(skill.requires_tools),
      }
    } else {
      resetForm()
    }
  }
  showCreateDialog.value = true
}

async function create() {
  if (createMode.value === 'github') {
    await installFromGithub()
    return
  }
  // manual mode: create or update
  installLoading.value = true
  try {
    const data = {
      name: form.value.name,
      description: form.value.description,
      description_en: form.value.description_en || '',
      category: form.value.category,
      author: form.value.author || '',
      version: form.value.version || '1.0.0',
      content_zh: form.value.content_zh,
      content_en: form.value.content_en || '',
      folder: form.value.folder || null,
      live_data: !!form.value.live_data,
      requires_tools: parseToolList(form.value.requires_tools),
    }
    if (editingSkill.value) {
      await skillsApi.update(editingSkill.value.name, data)
      ElMessage.success(t('skills.skillUpdatedMsg', { name: data.name }))
    } else {
      await skillsApi.create(data)
      ElMessage.success(t('skills.skillCreated', { name: data.name }))
    }
    showCreateDialog.value = false
    resetForm()
    await load()
  } catch (e) { error.value = e.message }
  finally { installLoading.value = false }
}

// 从 ZIP 解析 SKILL.md 填充表单
async function handleZipUpload(event) {
  const file = event.target.files?.[0]
  if (!file) return
  uploading.value = true
  uploadMsg.value = ''
  uploadStatus.value = null
  zipFileName.value = file.name
  try {
    const JSZip = (await import('jszip')).default
    const zip = await JSZip.loadAsync(file)
    const files = Object.keys(zip.files).filter(f => !zip.files[f].dir)
    let skillMd = null
    for (const f of files) {
      if (f.endsWith('SKILL.md') || f.endsWith('skill.md')) { skillMd = await zip.files[f].async('string'); break }
    }
    if (!skillMd) {
      for (const f of files) {
        if (f.endsWith('.md')) { skillMd = await zip.files[f].async('string'); break }
      }
    }
    if (!skillMd) {
      uploadMsg.value = t('skills.zipNoSkillMd')
      uploadStatus.value = 'error'
      return
    }
    const fm = parseFrontmatter(skillMd)
    let body = skillMd
    if (skillMd.startsWith('---')) {
      const end = skillMd.indexOf('\n---', 3)
      if (end >= 0) body = skillMd.slice(end + 4).trim()
    }
    const marker = '<!-- LANG:EN -->'
    const markerPos = body.indexOf(marker)
    if (fm.name) form.value.name = fm.name
    if (fm.description) form.value.description = fm.description
    if (fm.description_en) form.value.description_en = fm.description_en
    if (fm.category) form.value.category = fm.category
    if (fm.author) form.value.author = fm.author
    if (fm.version) form.value.version = fm.version
    // ★ 数据源声明必须跟着走。ZIP 里写好了却被这里丢掉的话，用户下一眼看到的是
    //   一个"声明没了"的表单，保存后 SKILL.md 里的声明就真的没了——
    //   正是这次事故的同款沉默丢失。键名与后端 parse_skill_file 一致，
    //   kebab 与 snake 两种写法都认。
    const fmLive = fm['live-data'] ?? fm['live_data']
    if (fmLive !== undefined) form.value.live_data = /^(true|yes|on|1)$/i.test(String(fmLive).trim())
    const fmTools = fm['requires-tools'] ?? fm['requires_tools']
    if (fmTools !== undefined) {
      form.value.requires_tools = String(fmTools).replace(/^\[|\]$/g, '').split(',').map(s => s.trim().replace(/^["']|["']$/g, '')).filter(Boolean).join(', ')
    }
    if (body) {
      form.value.content_zh = markerPos >= 0 ? body.substring(0, markerPos).trim() : body
      form.value.content_en = markerPos >= 0 ? body.substring(markerPos + marker.length).trim() : ''
    }
    uploadMsg.value = t('skills.zipParseSuccess')
    uploadStatus.value = 'success'
    // 切到 manual 模式确保表单可见
    createMode.value = 'manual'
    setTimeout(() => { uploadMsg.value = '' }, 3000)
  } catch (e) {
    uploadMsg.value = t('skills.zipParseFailed') + ': ' + (e?.message || '')
    uploadStatus.value = 'error'
  } finally {
    uploading.value = false
  }
}

function parseFrontmatter(md) {
  const fm = {}
  if (!md.startsWith('---')) return fm
  const rest = md.slice(3)
  const end = rest.indexOf('\n---')
  if (end < 0) return fm
  const text = rest.slice(0, end)
  for (const line of text.split('\n')) {
    const idx = line.indexOf(':')
    if (idx > 0) {
      let key = line.slice(0, idx).trim()
      let val = line.slice(idx + 1).trim()
      if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
        val = val.slice(1, -1)
      }
      fm[key] = val
    }
  }
  return fm
}

// Install from GitHub（改走 invoke 本地命令）
async function installFromGithub() {
  if (!githubUrl.value) {
    ElMessage.warning(t('skills.enterGitHubUrl'))
    return
  }
  installLoading.value = true
  try {
    await skillsApi.importGithub(githubUrl.value)
    showCreateDialog.value = false
    resetForm()
    await load()
    ElMessage.success(t('skills.skillInstallSuccess'))
  } catch (e) {
    ElMessage.error(t('skills.installFailed') + e.message)
  } finally {
    installLoading.value = false
  }
}

async function importFromFolder() {
  try {
    // Use Tauri dialog to pick a folder
    const { open } = await import('@tauri-apps/plugin-dialog')
    const dir = await open({ directory: true, multiple: false, title: '选择 Skill 文件夹' })
    if (!dir) return
    await skillsApi.importFolder(dir)
    await load()
    ElMessage.success(t('skills.importComplete'))
  } catch (e) { error.value = e.message }
}

async function importFile(event) {
  // Deprecated: single-file import replaced by folder import
  ElMessage.warning(t('skills.useFolderInstead'))
  event.target.value = ''
}

async function rebuildIndex() {
  rebuilding.value = true
  try {
    const res = await skillsApi.rebuildIndex()
    ElMessage.success(t('skills.rebuildResult', { indexed: res.indexed, total: res.total, failed: res.failed, duration: res.duration_ms }))
  } catch (e) { error.value = e.message }
  finally { rebuilding.value = false }
}

function resetForm() {
  form.value = { name: '', description: '', description_en: '', category: 'general', author: '', version: '1.0.0', content_zh: '', content_en: '', folder: '', live_data: false, requires_tools: '' }
}

async function viewDetail(item) {
  // 立即显示面板（同步），再异步加载完整数据
  showDetailDialog.value = true
  selectedItem.value = item
  fullSkill.value = null
  loadingDetail.value = true
  detailLang.value = 'zh'
  try {
    // 加超时保护：3 秒未响应则用列表数据
    const full = await Promise.race([
      skillsApi.get(item.name),
      new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 3000))
    ])
    fullSkill.value = full
    if (full) {
      selectedItem.value = { ...item, ...full }
    }
  } catch (_e) {
    // 超时或失败：使用列表数据作为回退
    console.warn('[SkillManager] detail load failed, using list data:', _e)
  } finally {
    loadingDetail.value = false
  }
}

function successRate(item) {
  const total = (item.success_count || 0) + (item.failure_count || 0)
  return total > 0 ? ((item.success_count || 0) / total * 100).toFixed(0) : '-'
}

// 详情编辑里 requires_tools 是数组，没法直接 v-model 到文本框，
// 用这个字符串中转（进入编辑时载入，保存时解析）
const detailToolList = ref('')

function enterDetailEdit() {
  detailEditing.value = true
  detailEditLang.value = detailLang.value
  detailError.value = ''
  detailToolList.value = joinToolList(selectedItem.value?.requires_tools)
  // 其余表单字段复用 selectedItem，在模板中直接绑定
}

function cancelDetailEdit() {
  detailEditing.value = false
  detailError.value = ''
}

async function saveDetailEdit() {
  if (!selectedItem.value?.name) return
  detailSaving.value = true
  detailError.value = ''
  try {
    const payload = {
      name: selectedItem.value.name,
      description: selectedItem.value.description,
      description_en: selectedItem.value.description_en || '',
      category: selectedItem.value.category,
      author: selectedItem.value.author || '',
      version: selectedItem.value.version || '1.0.0',
      content_zh: selectedItem.value.content_zh || '',
      content_en: selectedItem.value.content_en || '',
      folder: null,
      live_data: !!selectedItem.value.live_data,
      requires_tools: parseToolList(detailToolList.value),
    }
    await skillsApi.update(selectedItem.value.name, payload)
    detailEditing.value = false
    ElMessage.success(t('skills.skillUpdated'))
    await load({ silent: true })
  } catch (e) {
    detailError.value = e.message || t('skills.updateFailed')
  } finally {
    detailSaving.value = false
  }
}

function toggleSelect(id) {
  if (selectedIds.value.has(id)) selectedIds.value.delete(id)
  else selectedIds.value.add(id)
  selectedIds.value = new Set(selectedIds.value)
}

function toggleSelectAll() {
  if (selectedIds.value.size === sorted.value.length) {
    selectedIds.value = new Set()
  } else {
    selectedIds.value = new Set(sorted.value.map(i => i.name))
  }
}

async function batchToggle(enable) {
  const ids = [...selectedIds.value]
  const CONCURRENCY = 5
  const errors = []
  batchLoading.value = true

  for (let i = 0; i < ids.length; i += CONCURRENCY) {
    const batch = ids.slice(i, i + CONCURRENCY)
    const results = await Promise.allSettled(
      batch.map(id => {
        const item = items.value.find(s => s.name === id)
        if (item && item.enabled !== enable) return toggle(item, true)
        return Promise.resolve()
      })
    )
    results.forEach((r, j) => {
      if (r.status === 'rejected') {
        const skill = items.value.find(s => s.name === batch[j])
        errors.push(skill?.name || batch[j])
      }
    })
  }

  selectedIds.value = new Set()
  batchLoading.value = false

  if (errors.length > 0) {
    ElMessage.error(t('skills.batchToggleFailed') + errors.join('、'))
  }
}

async function batchDelete() {
  const ids = [...selectedIds.value]
  const count = ids.length
  if (!await confirmDialog(t('skills.confirmBatchDelete', { n: count }), { closeOnClickModal: false })) return
  const CONCURRENCY = 5
  const errors = []
  batchLoading.value = true

  for (let i = 0; i < ids.length; i += CONCURRENCY) {
    const batch = ids.slice(i, i + CONCURRENCY)
    const results = await Promise.allSettled(
      batch.map(id => skillsApi.delete(id).catch(e => { throw { id, error: e } }))
    )
    results.forEach((r, j) => {
      if (r.status === 'rejected') {
        errors.push(batch[j])
      }
    })
  }

  items.value = items.value.filter(i => !selectedIds.value.has(i.name))
  selectedIds.value = new Set()
  batchLoading.value = false

  if (errors.length > 0) {
    ElMessage.error(t('skills.batchDeleteFailed') + errors.join('、'))
  }
}

async function exportSkills() {
  // 导出为 ZIP，每个 Skill 一个 SKILL.md（含 frontmatter + 双语正文）
  const toExport = selectedIds.value.size > 0
    ? items.value.filter(i => selectedIds.value.has(i.name))
    : items.value
  if (!toExport.length) return

  try {
    const { default: JSZip } = await import('jszip')
    const zip = new JSZip()

    for (const item of toExport) {
      // 取得完整 skill 数据（含双语正文）
      let full = item
      try { full = await skillsApi.get(item.name) || item } catch { /* 使用列表数据 */ }

      // 构建 frontmatter + 正文
      let md = '---\n'
      md += `name: ${full.name || item.name}\n`
      if (full.description) md += `description: ${full.description}\n`
      if (full.description_en) md += `description_en: ${full.description_en}\n`
      if (full.category) md += `category: ${full.category}\n`
      if (full.author) md += `author: ${full.author}\n`
      if (full.version) md += `version: ${full.version}\n`
      if (full.tags?.length) md += `tags: [${full.tags.join(', ')}]\n`
      md += `enabled: ${full.is_enabled ?? full.enabled ?? true}\n`
      md += '---\n\n'

      // 双语正文（中 + <!-- LANG:EN --> 分隔英文）
      const zhBody = full.content_zh || full.content || ''
      const enBody = full.content_en || ''
      if (zhBody) md += zhBody + '\n\n'
      if (enBody) md += '<!-- LANG:EN -->\n' + enBody + '\n'
      else if (!zhBody) md += '# ' + full.name + '\n'

      zip.file((item.name || 'skill') + '/SKILL.md', md)
    }

    const blob = await zip.generateAsync({ type: 'blob' })

    // File System Access API 优先（弹出保存对话框）
    if ('showSaveFilePicker' in window) {
      try {
        const handle = await window.showSaveFilePicker({
          suggestedName: 'skills_export.zip',
          types: [{ description: 'ZIP', accept: { 'application/zip': ['.zip'] } }],
        })
        const writable = await handle.createWritable()
        await writable.write(blob)
        await writable.close()
      } catch (e) {
        if (e && e.name === 'AbortError') return
        throw e
      }
    } else {
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url; a.download = 'skills_export_' + new Date().toISOString().slice(0, 10) + '.zip'
      a.click(); URL.revokeObjectURL(url)
    }
  } catch (e) {
    ElMessage.error(t('skills.exportFailed') + (e && e.message ? e.message : String(e)))
  }
}

onMounted(load)
</script>

<template>
  <div class="skills-manager-root p-6 max-w-9xl mx-auto flex flex-col" :data-theme-id="themeStore.draftPreset.id" style="background: var(--bg-primary); height: 100%;" @click="showCategoryMenu = false; showSortMenu = false">
    <div class="flex items-start justify-between mb-3">
      <div>
        <div class="text-lg font-bold"><img :src="sdIcon" class="w-7 h-7 inline-block align-middle" /> {{ $t('skills.title') }}</div>
        <div class="text-base mt-1">{{ $t('skills.subtitle') }}</div>
      </div>
      <div class="flex items-center gap-2 flex-wrap justify-end">
        <button @click="importFromFolder" class="flex items-center gap-1.5 px-3 py-2 text-white text-sm rounded transition font-medium "
          style="background:var(--color-primary)"
          @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--color-primary)'">
          <Upload :size="15" /> {{ $t('skills.importSkill') }}
        </button>
        <button @click="exportSkills" class="flex items-center gap-1.5 px-3 py-2 text-white text-sm rounded transition font-medium"
          style="background:var(--btn-primary-bg)"
          @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--btn-primary-bg)'">
          <Download :size="15" /> {{ $t('skills.exportZip') }}
        </button>
        <!-- 重建索引已废弃：本地文件系统无需索引 -->
        <button @click="openCreateDialog('manual')" class="flex items-center gap-1.5 px-3 py-2 text-white text-sm rounded transition font-medium"
          style="background:var(--btn-primary-bg)"
          @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--btn-primary-bg)'">
          <Plus :size="15" /> {{ editingSkill ? $t('skills.newSkill') : $t('skills.newSkill') }}
        </button>
        <button @click="load" class="p-2 text-white rounded transition"
          style="background:var(--btn-primary-bg)"
          @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--btn-primary-bg)'">
          <RefreshCw :size="15" />
        </button>
      </div>
    </div>

    <div class="skills-stats-row" :style="darkTextStyle">
      <div class="skills-stats-card bg-white border border-gray-200 rounded-lg px-3 flex items-center gap-3 shadow-sm" :style="cardBgStyle">
        <div class="w-10 h-10 rounded-lg bg-indigo-50 flex items-center justify-center"><img :src="sdIcon" class="w-5 h-5" /></div>
          <div class="text-base text-gray-500">{{ $t('skills.allSkills') }}</div>
          <div class="text-3xl font-semibold text-gray-900">{{ stats.total }}</div>
      </div>
      <div class="skills-stats-card bg-white border border-gray-200 rounded-lg px-3 flex items-center gap-3 shadow-sm" :style="cardBgStyle">
        <div class="w-10 h-10 rounded-lg bg-emerald-50 text-emerald-600 flex items-center justify-center"><img :src="runningIcon" class="w-5 h-5" /></div>
          <div class="text-base text-gray-500">{{ $t('skills.enabled') }}</div>
          <div class="text-3xl  font-semibold text-gray-900">{{ stats.enabled }}</div>
      </div>
      <div class="skills-stats-card bg-white border border-gray-200 rounded-lg px-3 flex items-center gap-3 shadow-sm" :style="cardBgStyle">
        <div class="w-10 h-10  rounded-lg bg-violet-50 text-violet-600 flex items-center justify-center"><img :src="typeIcon" class="w-5 h-5" /></div>
          <div class="text-base text-gray-500">{{ $t('skills.category') }}</div>
          <div class="text-3xl  font-semibold text-gray-900">{{ stats.categories }}</div>
      </div>
      <div class="skills-stats-card bg-white border border-gray-200 rounded-lg px-3 flex items-center gap-3 shadow-sm" :style="cardBgStyle">
        <div class="w-10 h-10 rounded-lg bg-cyan-50 flex items-center justify-center"><img :src="scoreIcon" class="w-5 h-5" /></div>
          <div class="text-base text-gray-500">{{ $t('skills.recentWeekUsage') }}</div>
          <div class="text-3xl font-semibold text-gray-900">{{ stats.recentUsed ?? '—' }}</div>
      </div>
    </div>

    <div class="flex items-center gap-2 mb-5">
      <div class="relative flex-1 max-w-lg">
        <Search :size="15" class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 "  style="color: var(--text-secondary); "  />
        <input :value="searchQuery" @input="setSearchQuery(($event.target).value)" :placeholder="$t('skills.searchPlaceholder')"
          class="skills-search-input w-full pl-9 py-2.5 text-base rounded-lg outline-none"
          :class="searchQuery ? 'pr-9' : 'pr-3'"
          :style="searchInputStyle" />
        <SearchClearButton v-if="searchQuery" overlay @clear="setSearchQuery('')" />
      </div>
      <div class="relative">
        <button @click.stop="showCategoryMenu = !showCategoryMenu; showSortMenu = false" class="skills-filter-btn px-3 py-2 text-base rounded-lg inline-flex items-center gap-1.5" :style="filterBtnStyle">
          {{ categoryLabel }} <ChevronDown :size="14" />
        </button>
        <div v-if="showCategoryMenu" class="skills-filter-menu absolute right-0 mt-1 w-44 bg-white  rounded-lg shadow-lg z-50"  style="background: var(--theme-dailog-color) !important;">
          <button @click="categoryFilter = 'all'; showCategoryMenu = false" class="w-full text-left px-3 py-2 text-base hover:bg-gray-50">{{ $t('skills.allCategories') }}</button>
          <button v-for="c in categoriesWithCount" :key="c.value" @click="categoryFilter = c.value; showCategoryMenu = false" class="w-full text-left px-3 py-2 text-base hover:bg-gray-50">
            {{ c.label }}（{{ c.count }}）
          </button>
        </div>
      </div>
      <div class="relative">
        <button @click.stop="showSortMenu = !showSortMenu; showCategoryMenu = false" class="skills-filter-btn px-3 py-2 text-base rounded-lg inline-flex items-center gap-1.5" :style="filterBtnStyle">
          {{ $t('skills.sortBy') }}{{ sortLabel }} <ChevronDown :size="14" />
        </button>
        <div v-if="showSortMenu" class="skills-filter-menu absolute right-0 mt-1 w-56 bg-white border border-gray-200 rounded-lg shadow-lg z-50"  style="background: var(--theme-dailog-color) !important;">
          <button v-for="opt in sortOptions" :key="opt.value" @click="sortKey = opt.value; showSortMenu = false"
            class="w-full text-left px-3 py-2 text-base hover:bg-gray-50">
            {{ opt.label }}
          </button>
        </div>
      </div>
    </div>

    <!-- Error -->
    <div v-if="error" class="mb-4 p-3 bg-red-50 border border-red-200 text-red-700 text-base rounded-lg flex items-center gap-2">
      <span class="flex-1">{{ error }}</span>
      <button @click="error = ''" class="text-red-400 hover:text-red-600"><X :size="14" /></button>
    </div>

    <!-- Loading：透明背景，避免情景/黑暗主题下白底闪烁 -->
    <div v-if="loading" class="skills-loading-state flex-1 min-h-0 flex flex-col items-center justify-center gap-3 py-24">
      <Loader2 :size="32" class="animate-spin" style="color: var(--text-primary);" />
      <span class="text-sm font-medium" style="color: var(--text-secondary);">{{ $t('common.loading') }}</span>
    </div>

    <!-- Batch Actions -->
    <transition enter-active-class="transition-all duration-200" enter-from-class="opacity-0 -translate-y-2"
                leave-active-class="transition-all duration-150" leave-to-class="opacity-0 -translate-y-2">
      <div v-if="selectedIds.size > 0" class="mb-3 flex items-center gap-2 px-4 py-2.5 bg-[var(--card-background-tmd)] border border-[var(--border-color)] rounded-xl text-sm">
        <span class="font-medium">{{ $t('skills.selectedCount', { n: selectedIds.size }) }}</span>
        <button @click="batchToggle(true)" :disabled="batchLoading"
          class="px-3 py-2.5 bg-[var(--color-primary)] text-white text-md rounded hover:bg-[var(--btn-primary-hover-bg)] disabled:opacity-50">{{ $t('skills.batchEnable') }}</button>
        <button @click="batchToggle(false)" :disabled="batchLoading"
          class="px-3 py-2.5 bg-gray-500 text-white text-md rounded hover:bg-gray-600 disabled:opacity-50">{{ $t('skills.batchDisable') }}</button>
        <button @click="batchDelete" :disabled="batchLoading"
          class="px-3 py-2.5 bg-red-600 text-white text-md rounded hover:bg-red-700 disabled:opacity-50">{{ $t('skills.batchDelete') }}</button>
        <button @click="selectedIds = new Set()" class="ml-auto px-3 py-2.5 bg-gray-500 text-white text-md rounded-lg hover:bg-gray-600 disabled:opacity-50">{{ $t('skills.deselect') }}</button>
        <Loader2 v-if="batchLoading" :size="14" class="animate-spin text-indigo-500" />
      </div>
    </transition>

    <!-- 静默刷新进度条 -->
    <div v-if="refreshing && pagedItems.length > 0" class="h-0.5 bg-accent/30 rounded-full overflow-hidden mb-3">
      <div class="h-full bg-indigo-400 w-1/3 animate-shimmer rounded-full" />
    </div>

    <!-- Table：表头固定，仅表体滚动（与全部工具一致） -->
    <div v-if="!loading && sorted.length" class="table-card flex-1 min-h-0">
      <div ref="skillsTableHeadRef" class="skills-table-head">
        <table class="skills-table">
          <colgroup>
            <col class="col-check" />
            <col class="col-name" />
            <col class="col-info" />
            <col class="col-action" />
          </colgroup>
          <thead>
            <tr>
              <th class="col-check">
                <input type="checkbox" :checked="selectedIds.size === sorted.length && sorted.length > 0" @change="toggleSelectAll"
                  class="rounded border-gray-300 text-indigo-500 focus:ring-indigo-500" />
              </th>
              <th class="text-left">{{ $t('skills.columnName') }}</th>
              <th class="text-center col-info">{{ $t('skills.columnInfo') }}</th>
              <th class="text-center col-action">{{ $t('skills.columnAction') }}</th>
            </tr>
          </thead>
        </table>
      </div>
      <div ref="skillsTableBodyRef" class="skills-table-body-scroll" @scroll="onSkillsTableBodyScroll">
        <table class="skills-table">
          <colgroup>
            <col class="col-check" />
            <col class="col-name" />
            <col class="col-info" />
            <col class="col-action" />
          </colgroup>
          <tbody>
            <template v-for="item in pagedItems" :key="item.name">
              <tr>
              <td class="col-check">
                <input type="checkbox" :checked="selectedIds.has(item.name)" @change="toggleSelect(item.name)"
                  class="rounded border-gray-300 text-indigo-500 focus:ring-indigo-500" />
              </td>
              <td>
                <div class="flex items-center gap-3 min-w-0">
                  <div class="w-9 h-9 rounded-lg flex items-center justify-center text-sm flex-shrink-0">
                    <img v-if="svgCategories.has(item.category)" :src="categoryIcon(item.category)" class="w-5 h-5" />
                    <span v-else>{{ categoryIcon(item.category) }}</span>
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 min-w-0">
                      <span class="font-medium truncate" :title="item.name">{{ item.name }}</span>
                      <span v-if="item.category" class="inline-block rounded bg-indigo-50 px-1.5 py-0.5 text-xs text-indigo-600 flex-shrink-0">{{ catLabel(item.category) }}</span>
                    </div>
                    <div class="text-sm mt-0.5 truncate" :title="item.description">{{ item.description }}</div>
                  </div>
                </div>
              </td>
              <td class="col-info">
                <div class="skills-info-cell">
                  <span>{{ $t('skills.usageLabel') }}: {{ item.usage_count || 0 }}</span>
                </div>
              </td>
              <td class="text-right">
                <div class="flex items-center justify-center gap-1">
                  <button @click="toggle(item)" :disabled="pendingIds.has(item.name)" class="inline-flex items-center gap-1 disabled:opacity-50 p-1.5 rounded hover:bg-[var(--dropdown-hover-color)]" :title="item.enabled ? $t('skills.disable') : $t('skills.enable')">
                    <Loader2 v-if="pendingIds.has(item.name)" :size="16" class="animate-spin" />
                    <component v-else :is="item.enabled ? ToggleRight : ToggleLeft" :size="20" :class="item.enabled ? 'text-emerald-500' : 'text-gray-300'" />
                  </button>
                  <button @click="viewDetail(item)" class="p-1.5 rounded  hover:text-indigo-600 hover:bg-[var(--dropdown-hover-color)] transition-colors" :title="$t('skills.details')" style="cursor: pointer;">
                    <Eye :size="15" />
                  </button>
                  <button @click="remove(item)" class="p-1.5  hover:text-red-600 hover:bg-[var(--dropdown-hover-color)]" :title="$t('common.delete')">
                    <Trash2 :size="14" />
                  </button>
                </div>
              </td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>
    </div>

    <!-- ★ 分页控件 -->
    <div v-if="totalPages > 1" class="flex-shrink-0 flex items-center justify-between mt-4 px-1">
      <span class="text-sm" style="color: var(--text-secondary);">{{ $t('skills.paginationInfo', { filtered: filtered.length, page: currentPage, total: totalPages }) }}</span>
      <div class="flex items-center gap-1">
        <button :disabled="currentPage <= 1" @click="currentPage = currentPage - 1"
          class="px-2.5 py-1.5 text-sm rounded-lg border transition"
          :class="currentPage <= 1 ? 'opacity-40 cursor-not-allowed' : 'hover:bg-[var(--dropdown-hover-color)]'"
          style="border-color: var(--border-color); color: var(--text-primary);">‹</button>
        <template v-for="p in totalPages" :key="p">
          <button v-if="p === 1 || p === totalPages || Math.abs(p - currentPage) <= 1"
            @click="currentPage = p"
            class="px-2.5 py-1.5 text-sm rounded-lg transition"
            :class="p === currentPage ? 'text-white font-semibold' : 'hover:bg-[var(--dropdown-hover-color)]'"
            :style="p === currentPage ? { background: 'var(--color-primary)' } : { border: '1px solid var(--border-color)', color: 'var(--text-primary)' }">{{ p }}</button>
          <span v-else-if="p === 2 || p === totalPages - 1" class="px-1 text-sm" style="color: var(--text-secondary);">…</span>
        </template>
        <button :disabled="currentPage >= totalPages" @click="currentPage = currentPage + 1"
          class="px-2.5 py-1.5 text-sm rounded-lg border transition"
          :class="currentPage >= totalPages ? 'opacity-40 cursor-not-allowed' : 'hover:bg-[var(--dropdown-hover-color)]'"
          style="border-color: var(--border-color); color: var(--text-primary);">›</button>
      </div>
    </div>

    <!-- Empty -->
    <div v-if="!loading && !sorted.length" class="text-center py-20 ">
      <FileText :size="70" class="mx-auto mb-3" style="color: var(--text-primary);" />
      <p class="text-lg" style="color: var(--text-secondary);">{{ searchQuery ? $t('skills.noMatchSkills') : $t('skills.noSkills') }}</p>
      <button v-if="!searchQuery" @click="openCreateDialog('manual')" class="mt-3 inline-flex items-center gap-1.5 px-3 py-2 text-white text-base rounded font-medium" style="background:var(--btn-primary-bg)"
       @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
        @mouseleave="$event.currentTarget.style.background='var(--color-primary)'">
        <Plus :size="14" /> {{ $t('skills.createFirstSkill') }}
      </button>
    </div>

    <!-- Create Dialog 新建/编辑 Skill -->
    <Teleport to="body">
      <div v-if="showCreateDialog" class="fixed inset-0 z-50 flex items-center justify-center p-6" style="background: rgba(15, 23, 42, 0.15); backdrop-filter: blur(0px);">
        <div class="skills-create-dialog tool-form-dialog w-full max-w-lg mx-4 max-h-[86vh] overflow-y-auto" :data-theme-id="themeStore.draftPreset.id" style="background: linear-gradient(135deg, rgba(255,255,255,0.06), transparent), var(--theme-dailog-color); color: var(--text-primary); border-radius: 18px; border: 1px solid var(--border-color); box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);">
          <div class="flex items-center justify-between px-6 py-4" style="border-bottom: 1px solid var(--border-color);">
            <div class="flex items-center gap-2">
              <h2 style="color: var(--text-primary); font-size: var(--font-size-lg); font-weight: 700; margin: 0;">{{ editingSkill ? skillLabel(createLang, 'editSkill') : createMode === 'github' ? skillLabel(createLang, 'fromGithubImport') : skillLabel(createLang, 'newSkill') }}</h2>
              <div v-if="createMode !== 'github'" class="flex rounded-md border border-gray-200 overflow-hidden text-xs">
                <button type="button" @click="createLang = 'zh'" :class="createLang === 'zh' ? 'bg-[var(--color-primary)] text-white' : 'text-gray-500 hover:bg-[var(--dropdown-hover-color)]'" class="px-2 py-0.5">{{ skillLabel(createLang, 'chinese') }}</button>
                <button type="button" @click="createLang = 'en'" :class="createLang === 'en' ? 'bg-[var(--color-primary)] text-white' : 'text-gray-500 hover:bg-[var(--dropdown-hover-color)]'" class="px-2 py-0.5">{{ skillLabel(createLang, 'english') }}</button>
              </div>
            </div>
            <button @click="showCreateDialog = false"
              class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);"
              @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'"
              @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
              <X :size="20" />
            </button>
          </div>

          <!-- 模式切换 -->
          <div class="flex border-b border-gray-100 px-6 pt-2">
            <button @click="createMode = 'manual'"
              :class="createMode === 'manual' ? 'border-[var(--color-primary)] text-[var(--color-primary)]' : 'border-transparent text-gray-500 hover:text-gray-700'"
              class="px-4 py-2 text-base font-medium border-b-2 transition-colors">{{ skillLabel(createLang, 'manualCreateTab') }}</button>
            <button @click="createMode = 'github'"
              :class="createMode === 'github' ? 'border-[var(--color-primary)] text-[var(--color-primary)]' : 'border-transparent text-gray-500 hover:text-gray-700'"
              class="px-4 py-2 text-base font-medium border-b-2 transition-colors">{{ skillLabel(createLang, 'githubTab') }}</button>
          </div>

          <!-- GitHub 模式 -->
          <div v-if="createMode === 'github'" class="p-6 space-y-3">
            <div>
              <label class="mb-1 block text-sm font-medium">{{ skillLabel(createLang, 'repoUrl') }} *</label>
              <input v-model="githubUrl" placeholder="https://github.com/owner/repo"
                class="w-full rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm outline-none focus:border-[var(--color-primary)] focus:ring-2 focus:ring-[var(--color-primary)]/30" />
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div>
                <label class="mb-1 block text-sm font-medium">{{ skillLabel(createLang, 'subdir') }}</label>
                <input v-model="githubSubdir" placeholder="skills"
                  class="w-full rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm outline-none focus:border-[var(--color-primary)]" />
              </div>
              <div>
                <label class="mb-1 block text-sm font-medium">{{ skillLabel(createLang, 'branch') }}</label>
                <input v-model="githubRef" placeholder="main"
                  class="w-full rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm outline-none focus:border-[var(--color-primary)]" />
              </div>
            </div>
            <label class="flex items-center gap-2 text-sm text-gray-600">
              <input type="checkbox" v-model="githubOverwrite" class="rounded border-gray-300" />
              {{ skillLabel(createLang, 'overwriteExisting') }}
            </label>
          </div>

          <!-- Manual 模式 -->
          <div v-else class="p-6 space-y-3">
            <!-- ZIP 上传区（仅新建时显示） -->
            <div v-if="!editingSkill" class="rounded-lg border border-dashed border-gray-200 bg-gray-50/50 p-3 hover:bg-[var(--dropdown-hover-color)]">
              <label class="flex items-center gap-2 cursor-pointer text-sm text-gray-500 hover:text-gray-700 transition-colors ">
                <Upload :size="16" />
                <span v-if="uploading">{{ skillLabel(createLang, 'parsing') }}</span>
                <span v-else-if="zipFileName">{{ zipFileName }}</span>
                <span v-else>{{ skillLabel(createLang, 'uploadZipHint') }}</span>
                <input type="file" accept=".zip" class="hidden" @change="handleZipUpload($event)" />
              </label>
              <p v-if="uploadMsg" class="mt-1 text-xs" :class="uploadStatus === 'error' ? 'text-red-500' : 'text-green-600'">{{ uploadMsg }}</p>
            </div>

            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'name') }} <span class="text-red-400">*</span></label>
                <input v-model="form.name" :disabled="!!editingSkill" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none" :placeholder="skillLabel(createLang, 'namePlaceholder')" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'category') }}</label>
                <el-select v-model="form.category" :key="createLang" style="width: 100%;" class="skill-category-select">
                  <el-option v-for="c in categories" :key="c" :value="c" :label="popupCategoryLabel(createLang, c)">
                    {{ popupCategoryLabel(createLang, c) }}
                  </el-option>
                </el-select>
              </div>
            </div>

            <!-- 描述 -->
            <div v-if="createLang === 'zh'">
              <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'desc') }} <span class="text-red-400">*</span></label>
              <textarea v-model="form.description" rows="2" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none resize-none" :placeholder="skillLabel(createLang, 'descPlaceholder')" />
            </div>
            <div v-else>
              <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'descWithSuffix') }} <span class="text-gray-400 text-xs">{{ skillLabel(createLang, 'optional') }}</span></label>
              <textarea v-model="form.description_en" rows="2" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none resize-none" :placeholder="form.description || skillLabel(createLang, 'noEnglishDescPlaceholder')" />
            </div>

            <!-- 内容 -->
            <div v-if="createLang === 'zh'">
              <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'skillContent') }}<span class="text-red-400">*</span></label>
              <textarea v-model="form.content_zh" rows="6" class="w-full px-3 py-2 text-sm font-mono border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none resize-none" :placeholder="skillLabel(createLang, 'skillContentPlaceholder')" />
            </div>
            <div v-else>
              <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'skillContent') }} <span class="text-gray-400 text-xs">{{ skillLabel(createLang, 'optional') }}</span></label>
              <textarea v-model="form.content_en" rows="6" class="w-full px-3 py-2 text-sm font-mono border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none resize-none" :placeholder="form.content_zh ? skillLabel(createLang, 'noEnglishContentHint') : skillLabel(createLang, 'noEnglishContentPlaceholder')" />
            </div>

            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'author') }}</label>
                <input v-model="form.author" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none" :placeholder="skillLabel(createLang, 'optional')" />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-600 mb-1">{{ skillLabel(createLang, 'folder') }}（{{ skillLabel(createLang, 'optional') }}）</label>
                <input v-model="form.folder" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none" :placeholder="skillLabel(createLang, 'leaveEmptyAuto')" />
              </div>
            </div>

            <!-- 数据源声明：不填则按普通 skill 处理（无证据时只标注，不拦截） -->
            <div class="p-3 rounded-xl" style="background: var(--bg-hover);">
              <label class="flex items-center gap-2">
                <input type="checkbox" v-model="form.live_data" class="rounded border-gray-300" />
                <span class="text-sm font-medium">{{ skillLabel(createLang, 'liveData') }}</span>
              </label>
              <p class="text-xs mt-1 leading-relaxed" style="color: var(--text-secondary);">
                {{ skillLabel(createLang, 'liveDataHint') }}
              </p>

              <label class="block text-sm font-medium mb-1 mt-3" style="color: var(--text-secondary);">
                {{ skillLabel(createLang, 'requiresTools') }}
              </label>
              <input v-model="form.requires_tools"
                class="w-full px-3 py-2 text-sm font-mono border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none"
                :placeholder="skillLabel(createLang, 'requiresToolsPlaceholder')" />
              <p class="text-xs mt-1 leading-relaxed" style="color: var(--text-secondary);">
                {{ skillLabel(createLang, 'requiresToolsHint') }}
              </p>
            </div>
          </div>

          <div class="flex justify-end gap-3 px-6 py-4 rounded-b-2xl"
          style="border-top: 1px solid var(--border-color); background: var(--bg-hover);">
            <button @click="showCreateDialog = false"
              style="background: var(--cancel-btn-color); color: var(--btn-secondary-text); border: 1px solid var(--btn-secondary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600;"
              @mouseenter="$event.currentTarget.style.background='var(--cancel-btn-hover-color)';$event.currentTarget.style.color='var(--btn-secondary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--cancel-btn-color)';$event.currentTarget.style.color='var(--btn-secondary-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-border)'">{{ skillLabel(createLang, 'cancel') }}</button>
            <button @click="create"
              :disabled="installLoading || (createMode === 'manual' && !form.name) || (createMode === 'github' && !githubUrl)"
              :style="{ display: 'inline-flex', alignItems: 'center', background: 'var(--btn-primary-bg)', color: 'var(--btn-primary-text)', border: '1px solid var(--btn-primary-border)', minWidth: '76px', height: '36px', padding: '0 16px', borderRadius: '4px', cursor: (installLoading || (createMode === 'manual' && !form.name) || (createMode === 'github' && !githubUrl)) ? 'not-allowed' : 'pointer', fontSize: 'var(--font-size-base)', fontWeight: '600', transition: 'all 0.18s ease' }"
              @mouseenter="if (!(installLoading || (createMode === 'manual' && !form.name) || (createMode === 'github' && !githubUrl))) { $event.currentTarget.style.background='var(--btn-primary-hover-bg)';$event.currentTarget.style.color='var(--btn-primary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-primary-hover-border)' }"
              @mouseleave="if (!(installLoading || (createMode === 'manual' && !form.name) || (createMode === 'github' && !githubUrl))) { $event.currentTarget.style.background='var(--btn-primary-bg)';$event.currentTarget.style.color='var(--btn-primary-text)';$event.currentTarget.style.borderColor='var(--btn-primary-border)' }"
              class="px-4 py-2 text-sm text-white rounded-lg hover:bg-indigo-700 disabled:opacity-50 font-medium flex items-center gap-1">
              <Loader2 v-if="installLoading" :size="14" class="animate-spin" />
              <Check v-else :size="14" />
              {{ editingSkill ? skillLabel(createLang, 'update') : createMode === 'github' ? skillLabel(createLang, 'startImport') : skillLabel(createLang, 'create') }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- Detail Sheet (右侧抽屉) -->
    <Teleport to="body">
      <div v-if="showDetailDialog && selectedItem" class="fixed inset-0 z-[110]" style="background: rgba(15, 23, 42, 0.36);">
        <div class="fixed inset-y-0 right-0 w-[480px] max-w-[100vw] shadow-xl flex flex-col"  style="    background: linear-gradient(135deg, rgba(255, 255, 255, 0.06), transparent), var(--theme-dailog-color); color: var(--text-primary);">
          <!-- Header -->
          <div class="flex items-center justify-between px-5 py-4 border-b flex-shrink-0" style="border-color: var(--border-color);">
            <div>
              <h2 class="text-lg font-bold">{{ selectedItem.name }}</h2>
              <p class="text-sm mt-0.5" style="color: var(--text-secondary);">v{{ selectedItem.version }} · {{ popupCategoryLabel(detailLang, selectedItem.category) }}</p>
            </div>
            <div class="flex items-center gap-1">
              <button v-if="!detailEditing" @click="enterDetailEdit" class="p-1.5 rounded-lg hover:bg-[var(--dropdown-hover-color)]" style="color: var(--text-secondary);" :title="skillLabel(detailLang, 'edit')">
                <Edit3 :size="15" />
              </button>
              <button @click="showDetailDialog = false"
                class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);"
                @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'"
                @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
                <X :size="20" />
              </button>
            </div>
          </div>

          <!-- Body -->
          <div v-if="loadingDetail" class="flex-1 flex items-center justify-center">
            <Loader2 :size="24" class="animate-spin" style="color: var(--text-secondary);" />
          </div>
          <div v-else class="flex-1 overflow-y-auto p-5 space-y-4 text-sm">

            <!-- ──────── 查看模式 ──────── -->
            <template v-if="!detailEditing">
              <!-- 语言切换 -->
              <div class="">
                <button @click="detailLang = 'zh'" :class="detailLang === 'zh' ? 'bg-[var(--color-primary)] text-white shadow-sm font-semibold' : ''"
                  class="px-3 py-1 rounded-md text-xs transition">{{ skillLabel(detailLang, 'chinese') }}</button>
                <button @click="detailLang = 'en'" :class="detailLang === 'en' ? 'bg-[var(--color-primary)] text-white shadow-sm font-semibold' : ''"
                  class="px-3 py-1 rounded-md text-xs transition">{{ skillLabel(detailLang, 'english') }}</button>
              </div>

              <!-- 描述 -->
              <div>
                <span class="text-md font-semibold" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'desc') }}</span>
                <p class="mt-0.5 leading-relaxed">{{ detailLang === 'en' && selectedItem.description_en ? selectedItem.description_en : selectedItem.description }}</p>
              </div>

              <!-- 元信息网格 -->
              <div class="grid grid-cols-2 gap-3 p-3 rounded-xl" style="background: var(--bg-hover);">
                <div><span class="text-sm" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'status') }}</span>
                  <b class="block text-sm" :class="selectedItem.enabled ? 'text-green-600' : 'text-gray-400'">{{ selectedItem.enabled ? skillLabel(detailLang, 'enable') : skillLabel(detailLang, 'disable') }}</b></div>
                <div><span class="text-sm" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'usageCount') }}</span>
                  <span class="block text-sm">{{ selectedItem.usage_count ?? 0 }}</span></div>
                <div v-if="selectedItem.author"><span class="text-sm" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'author') }}</span>
                  <span class="block text-sm">{{ selectedItem.author }}</span></div>
                <div v-if="selectedItem.created_at"><span class="text-sm" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'createdAt') }}</span>
                  <span class="block text-sm">{{ selectedItem.created_at }}</span></div>
                <div v-if="selectedItem.updated_at"><span class="text-sm" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'updatedAt') }}</span>
                  <span class="block text-xs">{{ selectedItem.updated_at }}</span></div>
                <!-- 数据源声明：有没有声明决定拿不到数据时是硬拒还是只标注 -->
                <div><span class="text-sm" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'liveData') }}</span>
                  <b class="block text-sm" :class="selectedItem.live_data ? 'text-amber-600' : 'text-gray-400'">{{ selectedItem.live_data ? '✓' : '—' }}</b></div>
                <div v-if="(selectedItem.requires_tools || []).length"><span class="text-sm" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'requiresTools') }}</span>
                  <span class="block text-sm font-mono break-all">{{ (selectedItem.requires_tools || []).join(', ') }}</span></div>
              </div>

              <!-- 标签 -->
              <div v-if="(selectedItem.tags || []).length">
                <span class="text-md font-semibold" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'tags') }}</span>
                <div class="flex flex-wrap gap-1 mt-1">
                  <span v-for="tag in selectedItem.tags" :key="tag" class="inline-block px-2 py-0.5 rounded-full text-sm" style="background: var(--bg-hover);">{{ tag }}</span>
                </div>
              </div>

              <!-- SKILL.md 正文 -->
              <div>
                <span class="text-md font-semibold" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'skillContent') }}</span>
                <div class="mt-1 text-sm leading-relaxed rounded-xl px-4 py-3 max-h-96 overflow-y-auto border skill-content"
                  style="background: var(--bg-hover); border-color: var(--border-color);"
                  v-html="md.render(detailLang === 'en' && selectedItem.content_en ? selectedItem.content_en : (selectedItem.content_zh || skillLabel(detailLang, 'noContent')))" />
              </div>

              <!-- 原文链接 -->
              <div v-if="selectedItem.origin_url">
                <span class="text-xs font-semibold" style="color: var(--text-secondary);">{{ skillLabel(detailLang, 'source') }}</span>
                <a :href="selectedItem.origin_url" target="_blank" rel="noopener" class="block mt-0.5 text-xs text-indigo-600 hover:text-indigo-700 break-all underline underline-offset-2">{{ selectedItem.origin_url }}</a>
              </div>
            </template>

            <!-- ──────── 编辑模式 ──────── -->
            <template v-else>
              <div class="flex gap-1 bg-[var(--dropdown-hover-color)] rounded p-0.5 w-fit">
                <button @click="detailEditLang = 'zh'" :class="detailEditLang === 'zh' ? 'bg-[var(--color-primary)] text-white shadow-sm font-semibold' : ''"
                  class="px-3 py-1 rounded-md text-md transition">{{ skillLabel(detailEditLang, 'chinese') }}</button>
                <button @click="detailEditLang = 'en'" :class="detailEditLang === 'en' ? 'bg-[var(--color-primary)] text-white shadow-sm font-semibold' : ''"
                  class="px-3 py-1 rounded-md text-md transition">{{ skillLabel(detailEditLang, 'english') }}</button>
              </div>

              <div v-if="detailEditLang === 'zh'">
                <label class="text-md font-medium mb-1 block" style="color: var(--text-secondary);">{{ skillLabel(detailEditLang, 'descWithSuffix') }}</label>
                <input v-model="selectedItem.description" class="w-full px-3 py-2 text-sm border rounded-lg outline-none focus:border-[var(--color-primary)]" style="border-color: var(--border-color); color: var(--text-secondary); background: transparent;" />
              </div>
              <div v-else>
                <label class="text-md font-medium mb-1 block" style="color: var(--text-secondary);">{{ skillLabel(detailEditLang, 'descWithSuffix') }}</label>
                <input v-model="selectedItem.description_en" class="w-full px-3 py-2 text-sm border rounded outline-none focus:border-[var(--color-primary)] bg-transparent" style="border-color: var(--border-color);color: var(--text-secondary); background: transparent;"  />
              </div>

              <div>
                <label class="text-md font-medium mb-1 block" style="color: var(--text-secondary);">{{ skillLabel(detailEditLang, 'category') }}</label>
                <el-select v-model="selectedItem.category" :key="detailEditLang" class="skill-category-select" style="width: 100%;">
                  <el-option v-for="c in categories" :key="c" :value="c" :label="popupCategoryLabel(detailEditLang, c)">
                    {{ popupCategoryLabel(detailEditLang, c) }}
                  </el-option>
                </el-select>
              </div>

              <div v-if="detailEditLang === 'zh'">
                <label class="text-md font-medium mb-1 block" style="color: var(--text-secondary);background: transparent;">{{ skillLabel(detailEditLang, 'skillContent') }}</label>
                <textarea v-model="selectedItem.content_zh" rows="8" class="w-full px-3 py-2 text-sm font-mono border rounded-lg outline-none focus:border-[var(--color-primary)] resize-y" style="border-color: var(--border-color); background: transparent;" :placeholder="skillLabel(detailEditLang, 'skillContentPlaceholder')" />
              </div>
              <div v-else>
                <label class="text-md font-medium mb-1 block" style="color: var(--text-secondary);">{{ skillLabel(detailEditLang, 'skillContent') }}</label>
                <textarea v-model="selectedItem.content_en" rows="8" class="w-full px-3 py-2 text-sm font-mono border rounded-lg outline-none focus:border-[var(--color-primary)] resize-y" style="border-color: var(--border-color); background: transparent;" :placeholder="selectedItem.content_zh ? skillLabel(detailEditLang, 'noEnglishContentHint') : skillLabel(detailEditLang, 'noEnglishContentPlaceholder')" />
              </div>

              <label class="flex items-center gap-2">
                <input type="checkbox" v-model="selectedItem.enabled" class="rounded border-gray-300" />
                <span class="text-md">{{ skillLabel(detailEditLang, 'enable') }}</span>
              </label>

              <!-- 数据型声明：这两项决定系统敢不敢用这个 skill 的产出作答 -->
              <div class="pt-2 border-t" style="border-color: var(--border-color);">
                <label class="flex items-center gap-2">
                  <input type="checkbox" v-model="selectedItem.live_data" class="rounded border-gray-300" />
                  <span class="text-md font-medium">{{ skillLabel(detailEditLang, 'liveData') }}</span>
                </label>
                <p class="text-xs mt-1 leading-relaxed" style="color: var(--text-secondary);">
                  {{ skillLabel(detailEditLang, 'liveDataHint') }}
                </p>

                <label class="text-md font-medium mb-1 mt-3 block" style="color: var(--text-secondary);">
                  {{ skillLabel(detailEditLang, 'requiresTools') }}
                </label>
                <input v-model="detailToolList"
                  class="w-full px-3 py-2 text-sm font-mono border rounded-lg outline-none focus:border-[var(--color-primary)]"
                  style="border-color: var(--border-color); color: var(--text-secondary); background: transparent;"
                  :placeholder="skillLabel(detailEditLang, 'requiresToolsPlaceholder')" />
                <p class="text-xs mt-1 leading-relaxed" style="color: var(--text-secondary);">
                  {{ skillLabel(detailEditLang, 'requiresToolsHint') }}
                </p>
              </div>

              <p v-if="detailError" class="text-md text-red-500">{{ detailError }}</p>

              <div class="flex items-center gap-2 pt-2 justify-end">
                <button @click="cancelDetailEdit"
                  style="background: var(--cancel-btn-color); color: var(--btn-secondary-text); border: 1px solid var(--btn-secondary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; box-sizing: border-box;"
                  @mouseenter="$event.currentTarget.style.background='var(--cancel-btn-hover-color)';$event.currentTarget.style.color='var(--btn-secondary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-hover-border)'"
                  @mouseleave="$event.currentTarget.style.background='var(--cancel-btn-color)';$event.currentTarget.style.color='var(--btn-secondary-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-border)'"
                  >
                  {{ skillLabel(detailEditLang, 'cancel') }}</button>
                  <button @click="saveDetailEdit" :disabled="detailSaving"
                  style="display: inline-flex; align-items: center; justify-content: center; gap: 6px; background: var(--btn-primary-bg); color: var(--btn-primary-text); border: 1px solid var(--btn-primary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; box-sizing: border-box;"
                    @mouseenter="if (!detailSaving) { $event.currentTarget.style.background='var(--btn-primary-hover-bg)'; $event.currentTarget.style.color='var(--btn-primary-hover-text)'; $event.currentTarget.style.borderColor='var(--btn-primary-hover-border)' }"
                    @mouseleave="if (!detailSaving) { $event.currentTarget.style.background='var(--btn-primary-bg)'; $event.currentTarget.style.color='var(--btn-primary-text)'; $event.currentTarget.style.borderColor='var(--btn-primary-border)' }">
                  <Loader2 v-if="detailSaving" :size="14" class="animate-spin" />
                  {{ skillLabel(detailEditLang, 'save') }}
                </button>
              </div>
            </template>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
button { cursor: pointer; }

/* 统计卡片上下内边距：改这里的 8px 即可（四张一起生效） */
.skills-stats-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
  margin-bottom: 12px;
  align-items: start;
}

.skills-stats-card {
  padding-top: 8px !important;
  padding-bottom: 8px !important;
  align-self: start;
  box-sizing: border-box;
  min-width: 0;
  white-space: nowrap;
}

input[type="checkbox"] {
  appearance: none !important;
  -webkit-appearance: none !important;
  width: 16px !important;
  height: 16px !important;
  border: 1.5px solid var(--text-secondary) !important;
  border-radius: 4px !important;
  background: transparent !important;
  cursor: pointer !important;
  position: relative !important;
  vertical-align: middle;
  flex-shrink: 0;
  padding: 0 !important;
  margin: 0 !important;
}

input[type="checkbox"]:checked {
  background: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
}

input[type="checkbox"]:checked::after {
  content: '';
  position: absolute;
  left: 4px;
  top: 2px;
  width: 5px;
  height: 8px;
  border: solid #fff;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

input[type="checkbox"]:indeterminate {
  background: var(--color-primary) !important;
  border-color: var(--color-primary) !important;
}
.skill-content :deep(h1) { font-size: 1.25rem; }
.skill-content :deep(h2) { font-size: 1.1rem; }
.skill-content :deep(h3) { font-size: 1rem; }
.skill-content :deep(p), .skill-content :deep(li) { font-size: 0.875rem; }
.skill-content :deep(code) { font-size: 0.75rem; background: rgba(0,0,0,0.05); padding: 1px 4px; border-radius: 3px; }
.skill-content :deep(pre) { font-size: 0.75rem; background: rgba(0,0,0,0.04); padding: 10px; border-radius: 8px; overflow-x: auto; }
.skill-content :deep(pre code) { background: none; padding: 0; }
.dark [class*="shadow"]:where(.bg-white) {
  background: #1a1a1a !important;
  border-color: rgba(255,255,255,0.15) !important;
}
.dark [class*="shadow"]:where(.bg-white) button {
  color: #ffffff !important;
}
.dark [class*="shadow"]:where(.bg-white) button:hover {
  background: rgba(255,255,255,0.08) !important;
}
.light input[type="text"],
.light input:not([type]) {
  color: #1f2937;
  
}
/* Stat card labels/numbers in dark theme */
.dark .grid .shadow-sm .text-gray-500 {
  color: #cbd5e0 !important;
}
.dark .grid .shadow-sm .text-gray-900 {
  color: #ffffff !important;
}
/* Dropdown trigger buttons in dark theme: match search input, no hover change */
/* Search input and dropdown triggers in dark theme — 排除筛选栏，由下方统一透明样式控制 */
.dark .flex.items-center.gap-2.mb-5 input:not(.skills-search-input) {
  background: rgba(255,255,255,0.06);
  border-color: rgba(255,255,255,0.12);
  color: #ffffff;
}
.dark .flex.items-center.gap-2.mb-5 input::placeholder {
  color: #8896a7;
}
.dark .flex.items-center.gap-2.mb-5 > .relative > button:not(.skills-filter-btn):not(.skills-search-clear) {
  background: rgba(255,255,255,0.06);
  border-color: rgba(255,255,255,0.12);
  color: #ffffff;
}
.dark .flex.items-center.gap-2.mb-5 > .relative > button:not(.skills-filter-btn):not(.skills-search-clear):hover {
  background: rgba(255,255,255,0.06) !important;
}
/* Title in dark theme */
.dark .text-2xl.font-semibold.text-gray-900 { color: #ffffff !important; }



/* 情景主题（除雾灰调）：白色文字 + 搜索框/下拉背景黑色5% */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-2xl.font-semibold,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-sm.text-gray-500,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .grid .text-gray-900,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .grid .text-gray-500 { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) input:not(.skills-search-input) { background: rgba(255,255,255,0.10); color: #fff; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) input::placeholder { color: rgba(255,255,255,0.6); }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .relative > button:not(.skills-filter-btn):not(.skills-search-clear) { background: rgba(255,255,255,0.10) !important; border-color: rgba(255,255,255,0.15) !important; color: #fff !important; }

.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .shadow-lg button:hover { background: rgba(0,0,0,0.05) !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .border-gray-200 { border-color: rgba(255,255,255,0.15) !important; }

/* 浅色/雾灰调：搜索框/下拉黑色5%背景 — 排除筛选栏 */
.light [data-theme-id="simple-light"] input:not(.skills-search-input),
.light [data-theme-id="scene-graphite-cyan"] input:not(.skills-search-input) { background: rgba(0,0,0,0.05); }
.light [data-theme-id="simple-light"] .relative > button:not(.skills-filter-btn):not(.skills-search-clear) { background: transparent !important; }
.light [data-theme-id="scene-graphite-cyan"] .relative > button:not(.skills-filter-btn):not(.skills-search-clear) { background: rgba(0,0,0,0.05) !important; }

/* 浅色/雾灰调：黑色文字 */
/* 弹窗暗色主题 */
.dark .skills-create-dialog { background: #1a1a1a !important; border-color: rgba(255,255,255,0.1) !important; color: #fff; }
.dark .skills-create-dialog h2,
.dark .skills-create-dialog .text-gray-900,
.dark .skills-create-dialog .text-gray-600,
.dark .skills-create-dialog .text-gray-500,
.dark .skills-create-dialog .text-gray-400,
.dark .skills-create-dialog .text-gray-700,
.dark .skills-create-dialog label,
.dark .skills-create-dialog button { color: #fff !important; }
.dark .skills-create-dialog input,
.dark .skills-create-dialog textarea,
.dark .skills-create-dialog select,
.dark .skills-create-dialog .text-sm { color: #fff !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog label,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog button,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog .text-gray-600,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog .text-gray-500,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog .text-gray-700,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog input,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog textarea,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog select,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-create-dialog .text-sm { color: #fff !important; }
.dark .skills-create-dialog .border-gray-100 { border-color: rgba(255,255,255,0.1) !important; }
.dark .skills-create-dialog .bg-gray-50 { background: rgba(255,255,255,0.04) !important; }
.dark .skills-create-dialog .hover\:bg-gray-200:hover { background: rgba(255,255,255,0.1) !important; }
.dark .skills-create-dialog .hover\:bg-gray-100:hover { background: rgba(255,255,255,0.06) !important; }
.dark .skills-create-dialog input,
.dark .skills-create-dialog textarea,
.dark .skills-create-dialog select { background: rgba(255,255,255,0.08); color: #fff; border-color: rgba(255,255,255,0.12); }

.light [data-theme-id="simple-light"] .text-2xl.font-semibold,
.light [data-theme-id="simple-light"] .grid .text-gray-900,
.light [data-theme-id="simple-light"] .grid .text-gray-500,
.light [data-theme-id="scene-graphite-cyan"] .text-2xl.font-semibold,
.light [data-theme-id="scene-graphite-cyan"] .grid .text-gray-900,
.light [data-theme-id="scene-graphite-cyan"] .grid .text-gray-500 { color: #1f2937 !important; }

/* 分类/排序下拉窗口纯白背景 */
.skills-filter-menu { background: #fff !important; }
/* 分类下拉框背景透明 */
:deep(.skill-category-select .el-select__wrapper) { background: transparent !important;}

/* 表格 — 表头固定，仅表体滚动（与全部工具一致） */
.table-card {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: transparent;
}

.skills-table-head {
  flex-shrink: 0;
  background: var(--table-header-bg-color);
  overflow: hidden;
  padding-right: 6px;
  box-sizing: border-box;
}

.skills-table-body-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;
  padding-bottom: 2px;
}

.skills-loading-state {
  background: transparent !important;
}

.skills-table-body-scroll::-webkit-scrollbar { width: 6px; height: 6px; }
.skills-table-body-scroll::-webkit-scrollbar-track { background: transparent; }
.skills-table-body-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15));
  border-radius: 3px;
}

.skills-table {
  width: 100%;
  min-width: 560px;
  table-layout: fixed;
  border-collapse: collapse;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  background: transparent;
}

.skills-table .col-check { width: 48px; }
.skills-table .col-name { width: 36%; }
.skills-table .col-info { width: 32%; }
.skills-table .col-action { width: 32%; }

.skills-table thead th {
  background: var(--table-header-bg-color) !important;
  font-weight: 600;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  padding: 10px 16px;
  border: none;
  white-space: nowrap;
}

.skills-table thead th.col-check,
.skills-table tbody td.col-check {
  width: 48px;
  padding-left: 12px;
  padding-right: 8px;
}

.skills-table thead th.text-left { text-align: left; }
.skills-table thead th.text-center,
.skills-table thead th.col-info,
.skills-table thead th.col-action { text-align: center; }

.skills-table tbody td.col-info {
  text-align: center;
}

.skills-info-cell {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.skills-table tbody td {
  padding: 12px 16px;
  border: none;
  color: var(--text-primary);
  background-color: transparent;
  vertical-align: middle;
}

.skills-table tbody tr:nth-child(even) td {
  background-color: var(--card-background-tmd);
}

.skills-table tbody tr:hover td {
  background-color: var(--dropdown-hover-color);
}

.skills-table tbody tr:nth-child(even):hover td {
  background-color: var(--dropdown-hover-color);
}

</style>

<style>
.skills-create-dialog .skill-category-select .el-select__wrapper {
  background: transparent !important;
}

/* 新建/编辑 Skill 弹窗：取消所有输入框聚焦态（高亮边框/ring） */
.skills-create-dialog input:focus,
.skills-create-dialog textarea:focus,
.skills-create-dialog select:focus,
.skills-create-dialog .skill-category-select .el-select__wrapper.is-focused,
.skills-create-dialog .skill-category-select .el-select__wrapper:focus {
  outline: none !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  --tw-ring-shadow: 0 0 #0000 !important;
  --tw-ring-offset-shadow: 0 0 #0000 !important;
}

.light [data-theme-id="simple-light"] .skills-filter-btn,
.light [data-theme-id="scene-graphite-cyan"] .skills-filter-btn {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
}

/* Skills 筛选栏触发区（搜索框 + 下拉选中显示）：透明底 + --border-color，覆盖主题半透明底 */
html .skills-search-input,
html .skills-filter-btn,
.light .skills-search-input,
.light .skills-filter-btn,
.dark .skills-search-input,
.dark .skills-filter-btn,
.light [data-theme-id] .skills-search-input,
.light [data-theme-id] .skills-filter-btn,
.dark [data-theme-id] .skills-search-input,
.dark [data-theme-id] .skills-filter-btn {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  box-shadow: none !important;
}
html .skills-search-input,
.light .skills-search-input,
.dark .skills-search-input,
.light [data-theme-id] .skills-search-input,
.dark [data-theme-id] .skills-search-input {
  color: var(--text-primary) !important;
}
html .skills-search-input:focus,
html .skills-search-input:focus-visible,
html .skills-filter-btn:focus,
html .skills-filter-btn:focus-visible,
html .skills-filter-btn:hover,
.light [data-theme-id] .skills-search-input:focus,
.light [data-theme-id] .skills-search-input:focus-visible,
.light [data-theme-id] .skills-filter-btn:focus,
.light [data-theme-id] .skills-filter-btn:focus-visible,
.light [data-theme-id] .skills-filter-btn:hover,
.dark [data-theme-id] .skills-search-input:focus,
.dark [data-theme-id] .skills-search-input:focus-visible,
.dark [data-theme-id] .skills-filter-btn:focus,
.dark [data-theme-id] .skills-filter-btn:focus-visible,
.dark [data-theme-id] .skills-filter-btn:hover {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  outline: none !important;
}

/* 黑暗主题下文字变白 */
[data-theme-id="simple-dark"] table,
[data-theme-id="simple-dark"] .text-3xl,
[data-theme-id="simple-dark"] .text-2xl,
[data-theme-id="simple-dark"] h3,
[data-theme-id^="dark-"] table,
[data-theme-id^="dark-"] .text-3xl,
[data-theme-id^="dark-"] .text-2xl,
[data-theme-id^="dark-"] h3,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) table,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-3xl,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-2xl,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) h3 {
  color: #e5e7eb !important;
}

/* 情景主题（除雾灰调）下：仅 Skills 页内的开关/状态 emerald 文字变白，避免污染其它页面 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-manager-root .text-emerald-500,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .skills-manager-root .text-emerald-600
{
  color: #fff !important;
}
/* [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-gray-300,
[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-gray-400 {
  color: #d1d5db  !important;
} */
/* 黑暗主题下表格文字颜色 */
.dark table .text-gray-900 {
  color:rgb(210, 220, 241)!important;
}
.dark table .text-gray-500 {
  color: #9ca3af !important;
}
.dark table .text-gray-700 {
  color: #d1d5db !important;
}
.dark table .text-gray-400 {
  color: #6b7280 !important;
}
.dark table .text-indigo-600 {
  color: #818cf8 !important;
}
.dark table .text-emerald-600 {
  color: #34d399 !important;
}
.dark table .text-amber-600 {
  color: #fbbf24 !important;
}
.dark table .text-rose-600 {
  color: #f87171 !important;
}

</style>

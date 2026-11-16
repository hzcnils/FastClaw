<script setup lang="ts">
// FastClaw · WSP Local Microservice Management
import { onMounted, computed, ref, reactive, watch, onBeforeUnmount, onActivated, onDeactivated, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import { useWspStore } from '@/stores/wsp'
import { useMenuStore } from '@/stores/menu'
import { useThemeStore } from '@/stores/theme'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { ElMessageBox, ElMessage } from 'element-plus'
import type { DepServiceStatus } from '@/types'
import { useFastKgUpdate } from '@/composables/useFastKgUpdate'
import FastKgInstallDialog from '@/components/fastkg/FastKgInstallDialog.vue'
import { getDependencyRecoveryStatus, type DependencyRecoveryStatus } from '@/api/dependencyRecovery'
import SearchClearButton from '@/components/common/SearchClearButton.vue'
const { t } = useI18n()
const wsp = useWspStore()
const menuStore = useMenuStore()
const themeStore = useThemeStore()
const route = useRoute()
const showAddDialog = ref(false)
const showUploadDialog = ref(false)
const showEditDialog = ref(false)

// 新建服务表单
const newService = ref({
  name: '',
  display_name: '',
  description: '',
  category: '',
  category_name: '',
  sub_category_name: '',
  category_model: [] as string[],
  port: 8080,
  command: '',
  auto_start: false,
})

// 编辑服务表单
const editForm = ref({
  id: '',
  display_name: '',
  description: '',
  category_name: '',
  sub_category_name: '',
  category_model: [] as string[],
  port: 8080,
  command: '',
  auto_start: false,
  host: '',
  base_path: '',
})

// 启动中的服务 ID 集合（防止重复点击）
const startingIds = ref<Record<string, boolean>>({})

// 文件上传
const uploadFileName = ref<string>('')
const uploadFilePath = ref<string>('')
const uploadCategory = ref<string[]>([])
const uploadUrl = ref('')
const uploadDisplayName = ref('')
const isGgufUpload = computed(() => uploadFileName.value.toLowerCase().endsWith('.gguf'))
const canUpload = computed(() => !!(uploadFileName.value && uploadCategory.value?.length && (isGgufUpload.value || uploadUrl.value.trim())))
const canAddService = computed(() => !!(
  newService.value.display_name.trim() &&
  newService.value.category_model?.length &&
  newService.value.port &&
  newService.value.command.trim()
))
const isDragging = ref(false);
const timer = ref<any>(null);

// 打开弹窗时清空表单数据
watch(showUploadDialog, (val) => {
  if (val) {
    uploadFileName.value = ''
    uploadFilePath.value = ''
    uploadCategory.value = []
    uploadUrl.value = ''
    uploadDisplayName.value = ''
  }
})
watch(showAddDialog, (val) => {
  if (val) {
    newService.value = {
      name: '',
      display_name: '',
      description: '',
      category: '',
      category_name: '',
      sub_category_name: '',
      category_model: [],
      port: 8080,
      command: '',
      auto_start: false,
    }
  }
})

// 首次使用引导
const serviceBodyRef = ref<HTMLElement | null>(null)
const uploadServiceBtnRef = ref<HTMLElement | null>(null)
const uploadServiceEmptyBtnRef = ref<HTMLElement | null>(null)
const addServiceBtnRef = ref<HTMLElement | null>(null)
const addServiceEmptyBtnRef = ref<HTMLElement | null>(null)
const startAllBtnRef = ref<HTMLElement | null>(null)
const stopAllBtnRef = ref<HTMLElement | null>(null)

// 错误提示状态（自动消失）
const showErrorToast = ref(false)
const errorToastMessage = ref('')
let errorToastTimer: any = null

// 显示错误提示（2秒后自动消失）
function showError(message: string) {
  errorToastMessage.value = message
  showErrorToast.value = true
  if (errorToastTimer) clearTimeout(errorToastTimer)
  errorToastTimer = setTimeout(() => {
    showErrorToast.value = false
  }, 2000)
}

// 记录正在进行中的操作
const pendingActions = ref<Record<string, 'starting' | 'stopping'>>({})

// 处理服务启动
async function handleStartService(serviceId: string, serviceName: string) {
  pendingActions.value = { ...pendingActions.value, [serviceId]: 'starting' }
  try {
    await wsp.startService(serviceId)
    await wsp.loadServices()
    const svc = wsp.services.find((s: any) => s.id === serviceId)
    if (svc && (svc.runtime_status === 'error' || svc.runtime_status === 'crashed')) {
      const errorMsg = svc.runtime_error || t('wsp.startFailedUnknown')
      showError(`${serviceName} ${t('wsp.startFailed')}: ${errorMsg}`)
    }
  } catch (err: any) {
    const errorMsg = err?.message || String(err) || t('wsp.startFailedUnknown')
    showError(`${serviceName} ${t('wsp.startFailed')}: ${errorMsg}`)
  } finally {
    pendingActions.value = { ...pendingActions.value, [serviceId]: undefined as any }
  }
}

// 处理服务停止
async function handleStopService(serviceId: string, serviceName: string) {
  pendingActions.value = { ...pendingActions.value, [serviceId]: 'stopping' }
  try {
    await wsp.stopService(serviceId)
    await wsp.loadServices()
  } catch (err: any) {
    const errorMsg = err?.message || String(err) || t('wsp.stopFailedUnknown')
    showError(`${serviceName} ${t('wsp.stopFailed')}: ${errorMsg}`)
  } finally {
    pendingActions.value = { ...pendingActions.value, [serviceId]: undefined as any }
  }
}

// ── 首次使用引导 ──
const showGuide = ref(false)
const currentGuideStep = ref(0)
const guideTargetRect = ref<DOMRect | null>(null)

const guideSteps: Array<{ title: string; content: string; target: () => any }> = [
  {
    get title() { return t('wsp.guideStep1Title') },
    get content() { return t('wsp.guideStep1Content') },
    target: () => uploadServiceEmptyBtnRef.value || uploadServiceBtnRef.value,
  },
  {
    get title() { return t('wsp.guideStep2Title') },
    get content() { return t('wsp.guideStep2Content') },
    target: () => addServiceEmptyBtnRef.value || addServiceBtnRef.value,
  },
  {
    get title() { return t('wsp.guideStep3Title') },
    get content() { return t('wsp.guideStep3Content') },
    target: () => startAllBtnRef.value,
  },
  {
    get title() { return t('wsp.guideStep4Title') },
    get content() { return t('wsp.guideStep4Content') },
    target: () => stopAllBtnRef.value,
  },
]

function updateGuidePosition() {
  nextTick(() => {
    const target = guideSteps[currentGuideStep.value].target()
    console.log('[WSP Guide] target element:', target)
    guideTargetRect.value = target ? target.getBoundingClientRect() : null
  })
}

function startGuide() {
  console.log('[WSP Guide] startGuide called')
  showGuide.value = true
  currentGuideStep.value = 0
  updateGuidePosition()
}

function nextGuideStep() {
  if (currentGuideStep.value < guideSteps.length - 1) {
    currentGuideStep.value++
    updateGuidePosition()
  }
}

function prevGuideStep() {
  if (currentGuideStep.value > 0) {
    currentGuideStep.value--
    updateGuidePosition()
  }
}

async function closeGuide() {
  showGuide.value = false
  await invoke('update_setting', { key: 'wsp_startGuide', value: '1' })
}

const tooltipStyle = computed(() => {
  if (!guideTargetRect.value) return {}
  const rect = guideTargetRect.value
  const width = 320
  const gap = 16
  let left = rect.left + rect.width / 2 - width / 2
  left = Math.max(16, Math.min(left, window.innerWidth - width - 16))
  return {
    top: rect.bottom + gap + 'px',
    left: left + 'px',
    width: width + 'px',
  }
})

const tooltipArrowStyle = computed(() => {
  if (!guideTargetRect.value || !tooltipStyle.value.left) return {}
  const rect = guideTargetRect.value
  const tooltipLeft = parseInt(tooltipStyle.value.left as string, 10)
  const targetCenter = rect.left + rect.width / 2
  return {
    top: '-6px',
    left: targetCenter - tooltipLeft - 6 + 'px',
  }
})

// ─── 依赖服务管理 ───
const activeTab = ref(route.query.tab === 'deps' ? 'deps' : 'wsp');
watch(() => route.query.tab, tab => { if (tab === 'deps') activeTab.value = 'deps' })
const depServices = ref<DepServiceStatus[]>([]);
const depRecovery = ref<Record<string, DependencyRecoveryStatus>>({})
const globalRecoverySaving = ref(false)
const recoveryHistoryOpen = ref(false)
const recoveryDiagnostics = ref<string[]>([])
const recoveryHistory = ref<{ phase: string; attempt: number; detail: string | null; created_at: string }[]>([])
async function showRecoveryHistory(id: string) {
  try {
    recoveryHistory.value = await invoke('get_dependency_recovery_history', { serviceId: id })
    recoveryDiagnostics.value = await invoke('get_dependency_diagnostics', { serviceId: id })
    recoveryHistoryOpen.value = true
  } catch (error: any) { ElMessage.error(error?.message || String(error)) }
}
async function saveGlobalRecovery(value: boolean | string | number) {
  if (globalRecoverySaving.value) return
  globalRecoverySaving.value = true
  try {
    await invoke('set_dependency_recovery_settings', { enabled: Boolean(value) })
    for (const state of Object.values(depRecovery.value)) {
      state.global_enabled = Boolean(value)
      state.enabled = Boolean(value) && state.service_enabled
    }
    await loadDepServices()
  }
  catch (error: any) { ElMessage.error(error?.message || String(error)) }
  finally { globalRecoverySaving.value = false }
}
function recoveryText(id: string) {
  const state = depRecovery.value[id]
  if (!state || state.desired_state === 'stopped' || state.recovery_state === 'healthy') return ''
  const label: Record<string, string> = {
    queued: '等待启动', starting: `正在启动（第 ${state.recovery_attempt}/${state.max_recovery_attempts} 次）`,
    backoff: `将在 ${Math.ceil((state.retry_in_ms || 0) / 1000)} 秒后重试`,
    waiting_dependency: '等待上游服务就绪', waiting_shared_owner: '等待共享服务恢复或消费者升级',
    paused: '自动恢复已暂停', failed: '',
  }
  return label[state.recovery_state] || ''
}
async function recoveryAction(id: string, action: 'retry' | 'pause') {
  if (depBatchBusy.value) return
  try {
    await invoke(action === 'retry' ? 'retry_dependency_service' : 'pause_dependency_recovery', { serviceId: id })
    await loadDepServices()
  } catch (error: any) { ElMessage.error(error?.message || String(error)) }
}
async function saveRecoverySetting(id: string, key: 'enabled' | 'autoStart', value: boolean | string | number) {
  try {
    await invoke('set_dependency_recovery_settings', { serviceId: id, [key]: Boolean(value) })
    await loadDepServices()
  } catch (error: any) { ElMessage.error(error?.message || String(error)) }
}
const depStartingIds = ref(new Set<string>());
const depStoppingIds = ref(new Set<string>());
const depStartingAll = ref(false);
const depStoppingAll = ref(false);
const depBatchBusy = computed(() => depStartingAll.value || depStoppingAll.value);
const depIndividualBusy = computed(() => depStartingIds.value.size > 0 || depStoppingIds.value.size > 0);
let depPollTimer: ReturnType<typeof setInterval> | null = null;
let unlistenDepPortState: (() => void) | null = null;
type DepPortAttemptState = {
  phase: string
  preferredPort: number
  candidatePort?: number | null
  effectivePort?: number | null
  attempt: number
  message: string
}
const depPortStates = reactive<Record<string, DepPortAttemptState>>({});

function getDepPortStatusText(svc: DepServiceStatus, state: DepPortAttemptState) {
  const port = state.candidatePort || state.effectivePort || svc.port
  switch (state.phase) {
    case 'probing':
      return `正在检查候选端口 ${port} 是否可用（第 ${state.attempt} 个候选端口）`
    case 'occupied':
      return `端口 ${port} 已被占用，正在继续查找可用端口`
    case 'configuring':
      return `已选定端口 ${port}，正在同步 ${svc.name} 配置`
    case 'starting':
      return `已选定端口 ${port}，正在启动 ${svc.name}`
    case 'retrying':
      return `端口状态发生变化，正在为 ${svc.name} 重新选择可用端口`
    default:
      return state.message
  }
}

function getDepPortFallbackText(svc: DepServiceStatus) {
  const state = depPortStates[svc.id]
  if (state && ['probing', 'occupied'].includes(state.phase)) {
    return `首选端口 ${svc.preferred_port} 不可用，正在自动选择端口`
  }
  if (state && ['configuring', 'starting'].includes(state.phase)) {
    return `首选端口 ${svc.preferred_port} 不可用，已选择端口 ${svc.port}`
  }
  return `首选端口 ${svc.preferred_port} 不可用，当前使用端口 ${svc.port}`
}

function getDepServiceStatusLabel(svc: DepServiceStatus) {
  if (!svc.installed) return t('wsp.status_not_downloaded')
  // 手动启动进行中时，即使轮询短暂落到 failed，也保持「启动中」，避免误报
  if (depStartingIds.value.has(svc.id) && ['failed', 'stopped', 'unhealthy'].includes(svc.lifecycle_state)) {
    return '启动中'
  }
  switch (svc.lifecycle_state) {
    case 'port_occupied': return '端口占用'
    case 'probing': return '检查端口'
    case 'starting': return '启动中'
    case 'running': return svc.ownership === 'shared_managed' ? '共享运行' : '运行中'
    case 'unhealthy': return '服务异常'
    case 'stopping': return '停止中'
    case 'failed': return '启动失败'
    default: return '未启动'
  }
}

function getDepServiceTagType(svc: DepServiceStatus) {
  if (!svc.installed) return 'info'
  if (depStartingIds.value.has(svc.id) && ['failed', 'stopped', 'unhealthy'].includes(svc.lifecycle_state)) {
    return 'warning'
  }
  if (svc.lifecycle_state === 'running') return 'success'
  if (['port_occupied', 'probing', 'starting', 'stopping'].includes(svc.lifecycle_state)) return 'warning'
  if (['unhealthy', 'failed'].includes(svc.lifecycle_state)) return 'danger'
  return 'info'
}

// 编辑配置
const depEditDialogVisible = ref(false);
const depEditServiceId = ref('');
const depEditForm = reactive({ host: '', port: 0 });
const fastKgInstallDialogVisible = ref(false)

async function loadDepServices() {
  try {
    const [result, recovery] = await Promise.all([
      invoke<DepServiceStatus[]>('get_dependency_services_status'), getDependencyRecoveryStatus(),
    ])
    depRecovery.value = Object.fromEntries(recovery.map(item => [item.id, item]))
    depServices.value = result
  } catch (e) { console.warn('[WSP] 加载依赖服务状态失败', e); }
}

async function startDep(id: string) {
  if (depBatchBusy.value || depStartingIds.value.has(id) || depStoppingIds.value.has(id)) return
  const nextIds = new Set(depStartingIds.value);
  nextIds.add(id);
  depStartingIds.value = nextIds;
  try {
    await invoke('start_dependency_service', { serviceId: id });
    await loadDepServices();
    const svc = depServices.value.find(d => d.id === id);
    if (svc?.running) ElMessage.success(`${svc.name || id} 已启动`);
  } catch (e: any) {
    ElMessage.error(`启动失败：${e?.message || e}`);
  } finally {
    const cleared = new Set(depStartingIds.value);
    cleared.delete(id);
    depStartingIds.value = cleared;
    await loadDepServices();
  }
}

async function stopDep(id: string) {
  if (depBatchBusy.value || depStartingIds.value.has(id) || depStoppingIds.value.has(id)) return
  const nextIds = new Set(depStoppingIds.value);
  nextIds.add(id);
  depStoppingIds.value = nextIds;
  try {
    const message = await invoke<string>('stop_dependency_service', { serviceId: id });
    ElMessage.success(message);
  } catch (e: any) {
    ElMessage.error(`停止失败：${e?.message || e}`);
  } finally {
    const cleared = new Set(depStoppingIds.value);
    cleared.delete(id);
    depStoppingIds.value = cleared;
    await loadDepServices();
  }
}

function hasOtherKgConsumers(svc: DepServiceStatus) {
  if (!['qdrant', 'llama_server', 'fastkg'].includes(svc.id)) return false
  const consumers = svc.consumers || []
  return svc.ownership === 'external_unmanaged' || consumers.some(app => app !== 'fastclaw') || consumers.length > 1
}

async function startAllDepServices() {
  if (depBatchBusy.value || depIndividualBusy.value) return
  depStartingAll.value = true;
  try {
    const failures: string[] = []
    const skipped: string[] = []
    if (!depServices.value.length) await loadDepServices()
    for (const id of ['qdrant', 'llama_server', 'fastkg', 'fastplan']) {
      const service = depServices.value.find((item) => item.id === id)
      if (service && !service.installed) {
        skipped.push(service.name || id)
        continue
      }
      try {
        await invoke('start_dependency_service', { serviceId: id })
      } catch (error: any) {
        failures.push(`${id}: ${error?.message || error}`)
      }
    }
    if (failures.length) throw new Error(failures.join('；'))
    if (skipped.length) ElMessage.warning(`已跳过未下载的服务：${skipped.join('、')}`)
    else ElMessage.success('已提交全部依赖服务启动请求，请等待健康验证');
    await loadDepServices();
  } catch (e: any) {
    ElMessage.error(`启动失败：${e?.message || e}`);
  } finally {
    depStartingAll.value = false;
  }
}

async function stopAllDepServices() {
  if (depBatchBusy.value || depIndividualBusy.value) return
  depStoppingAll.value = true
  try {
    const message = await invoke<string>('stop_all_dependency_services')
    ElMessage.success(message)
  } catch (error: any) {
    ElMessage.error(`停止失败：${error?.message || error}`)
  } finally {
    await loadDepServices()
    depStoppingAll.value = false
  }
}

function openDepEditDialog(svc: DepServiceStatus) {
  depEditServiceId.value = svc.id;
  depEditForm.host = svc.host;
  depEditForm.port = svc.preferred_port || svc.port;
  depEditDialogVisible.value = true;
}

async function saveDepConfig() {
  if (!depEditServiceId.value) return;
  try {
    await invoke('update_dependency_service_config', {
      serviceId: depEditServiceId.value,
      host: depEditForm.host,
      port: depEditForm.port,
    });
    ElMessage.success(t('wsp.configWillRestart'));
    depEditDialogVisible.value = false;
  } catch (e: any) {
    ElMessage.error(`配置更新失败：${e?.message || e}`);
  } finally {
    await loadDepServices();
  }
}

const fastKgInstalled = computed(() => {
  const svc = depServices.value.find((d) => d.id === 'fastkg')
  return !!svc?.installed
})

const {
  localVersion: fastKgLocalVersion,
  remoteVersion: fastKgRemoteVersion,
  dialogVisible: fastKgUpdateDialogVisible,
  dialogMode: fastKgUpdateDialogMode,
  isUploading: fastKgUploading,
  uploadProgress: fastKgUploadProgress,
  uploadMessage: fastKgUploadMessage,
  showOnlineUpdateBtn: showFastKgOnlineUpdateBtn,
  showOfflineUploadBtn: showFastKgOfflineUploadBtn,
  kgDownloadInProgress: fastKgUpdateBusy,
  onEnterWspPage: onEnterFastKgVersionCheck,
  onLeaveWspPage: onLeaveFastKgVersionCheck,
  checkFastKgVersion,
  openOnlineUpdateDialog: openFastKgOnlineUpdate,
  openOfflineUploadDialog: openFastKgOfflineUpload,
  confirmOnlineUpdate: confirmFastKgOnlineUpdate,
  confirmOfflineUpload: confirmFastKgOfflineUpload,
} = useFastKgUpdate({
  isInstalled: fastKgInstalled,
  onAfterUpdate: async () => {
    // 覆盖后的启动、重试均由宿主 InstallHook 交给后端恢复管理器。
    await loadDepServices()
  },
})

async function handleFastKgInstalled() {
  await loadDepServices()
  if (!fastKgInstalled.value) {
    ElMessage.error(t('wsp.fastkgInstallIncomplete'))
    return
  }
  try {
    ElMessage.info(t('wsp.fastkgInstallStarting'))
    // 安装包同时包含 Qdrant、嵌入服务与 FastKG；安装完成后必须走完整编排，
    // 不能只刷新卡片，否则会继续展示安装前遗留的失败状态。
    await invoke('start_knowledge_service')
    await loadDepServices()
    ElMessage.success(t('wsp.fastkgInstallCompleted'))
  } catch (error: any) {
    await loadDepServices()
    const message = error?.message || String(error)
    console.warn('[WSP] FastKG 安装后自动启动失败:', message)
    ElMessage.error(t('wsp.fastkgInstallStartFailed', { msg: message }))
  } finally {
    await checkFastKgVersion()
  }
}

watch(fastKgInstalled, (installed, wasInstalled) => {
  if (installed && !wasInstalled) void checkFastKgVersion()
})

async function refreshFastKgVersionCheck() {
  await loadDepServices()
  await checkFastKgVersion()
}

// Tab 切换 → 控制依赖服务轮询；进入依赖服务时立即检查 FastKG 版本
watch(activeTab, (tab) => {
  if (depPollTimer) { clearInterval(depPollTimer); depPollTimer = null; }
  if (tab === 'deps') {
    void refreshFastKgVersionCheck();
    depPollTimer = setInterval(loadDepServices, 5000);
  }
});

// keep-alive 停用时清理
onDeactivated(() => {
  if (depPollTimer) { clearInterval(depPollTimer); depPollTimer = null; }
  onLeaveFastKgVersionCheck();
});

onMounted(() => {
  void refreshFastKgVersionCheck().then(() => onEnterFastKgVersionCheck());
  listen<DepPortAttemptState & { serviceId: string }>('dependency-service-port-state', (event) => {
    const payload = event.payload
    depPortStates[payload.serviceId] = payload
    const service = depServices.value.find(item => item.id === payload.serviceId)
    const displayPort = payload.effectivePort || payload.candidatePort
    if (service && displayPort) {
      service.port = displayPort
      service.using_fallback = displayPort !== payload.preferredPort
    }
    if (payload.phase === 'ready' || payload.phase === 'failed') void loadDepServices()
  }).then(unlisten => { unlistenDepPortState = unlisten }).catch(error => {
    console.warn('[WSP] 监听依赖服务端口状态失败', error)
  })
  menuStore.loadMenus()
  wsp.loadServices().then(async () => {
    console.log('[WSP Guide] services count:', wsp.services.length)
    if (wsp.services.length === 0) {
      try {
        const settings = await invoke<{ key: string; value: string }[]>('get_settings')
        const guideSetting = settings.find((s: any) => s.key === 'wsp_startGuide')
        if (!guideSetting || guideSetting.value === '0') {
          console.log('[WSP Guide] starting guide...')
          startGuide()
        }
      } catch { /* ignore, fallback to no guide */ }
    }
  })
  timer.value && clearInterval(timer.value);
  // 每 5 秒轮询一次，用于检测服务崩溃/外部停止等状态变化。
  // 当前架构无后端推送机制，轮询是唯一感知变化的方式，保留必要。
  timer.value = setInterval(() => {
    if (!wsp.isLoading) wsp.loadServices()
  }, 5000)
  window.addEventListener('resize', updateGuidePosition)
  serviceBodyRef.value?.addEventListener('scroll', updateGuidePosition)
})
onActivated(() => {
  void refreshFastKgVersionCheck().then(() => onEnterFastKgVersionCheck());
  if (activeTab.value === 'deps') {
    if (!depPollTimer) depPollTimer = setInterval(loadDepServices, 5000);
  }
  timer.value && clearInterval(timer.value);
  if (!wsp.isLoading) wsp.loadServices()
  timer.value = setInterval(() => {
    if (!wsp.isLoading) wsp.loadServices()
  }, 5000)
})
onBeforeUnmount(() => {
  onLeaveFastKgVersionCheck();
  if (depPollTimer) { clearInterval(depPollTimer); depPollTimer = null; }
  timer.value && clearInterval(timer.value);
  window.removeEventListener('resize', updateGuidePosition)
  serviceBodyRef.value?.removeEventListener('scroll', updateGuidePosition)
  unlistenDepPortState?.()
  unlistenDepPortState = null
})
onDeactivated(() => {
  timer.value && clearInterval(timer.value);
})

const statusColor = (status: string | null) => {
  switch (status) {
    case 'running': return 'var(--color-success)'
    case 'starting': return 'var(--color-warning)'
    case 'error': case 'crashed': return 'var(--color-danger)'
    default: return 'var(--text-secondary)'
  }
}

const statusLabel = (status: string | null) => {
  return t('wsp.status_' + (status || 'stopped'))
}

const statusTagType = (status: string | null): '' | 'success' | 'warning' | 'danger' | 'info' => {
  switch (status) {
    case 'running': return 'success'
    case 'starting': case 'stopping': return 'warning'
    case 'error': case 'crashed': return 'danger'
    default: return 'info'
  }
}

const runningCount = computed(() => wsp.services.filter(s => s.runtime_status === 'running').length)

// 搜索 & 筛选 & 视图模式
const searchName = ref('')
const filterStatus = ref('')
const viewMode = ref<'card' | 'table'>('card')
const wspTableHeadRef = ref<HTMLElement | null>(null)
const wspTableBodyRef = ref<HTMLElement | null>(null)

function onWspTableBodyScroll() {
  const head = wspTableHeadRef.value
  const body = wspTableBodyRef.value
  if (head && body) head.scrollLeft = body.scrollLeft
}

const statusOptions = computed(() => [
  { value: 'running', label: t('wsp.status_running'), tagType: 'success' as const },
  { value: 'starting', label: t('wsp.status_starting'), tagType: 'warning' as const },
  { value: 'stopped', label: t('wsp.status_stopped'), tagType: 'info' as const },
  { value: 'error', label: t('wsp.status_error'), tagType: 'danger' as const },
  { value: 'crashed', label: t('wsp.status_crashed'), tagType: 'danger' as const },
])

const filteredServices = computed(() => {
  return wsp.services.filter(s => {
    const nameMatch = !searchName.value ||
      (s.display_name || '').toLowerCase().includes(searchName.value.toLowerCase())
    const statusMatch = !filterStatus.value || s.runtime_status === filterStatus.value
    return nameMatch && statusMatch
  })
})

// 启动服务（带 loading 防重复点击）
const handleStart = async (serviceId: string) => {
  startingIds.value[serviceId] = true
  try {
    await wsp.startService(serviceId)
  } finally {
    delete startingIds.value[serviceId]
  }
}

// 打开编辑弹窗，预填当前服务数据
const openEditDialog = (svc: any) => {
  editForm.value = {
    id: svc.id,
    display_name: svc.display_name || '',
    description: svc.description || '',
    category_name: svc.category_name || '',
    sub_category_name: svc.sub_category_name || '',
    category_model: [svc.category_name, svc.sub_category_name].filter(Boolean) as string[],
    port: svc.port || 8080,
    command: svc.exe_path || '',
    auto_start: svc.auto_start || false,
    host: svc.host || '',
    base_path: svc.base_path || '',
  }
  showEditDialog.value = true
}

// 编辑弹窗分类级联变化
const onEditCategoryChange = (val: string[]) => {
  editForm.value.category_name = (val && val[0]) || ''
  editForm.value.sub_category_name = (val && val[1]) || ''
}

const isEditExeService = computed(() => /[\\/]/.test(editForm.value.command))
const isEditGgufService = computed(() => editForm.value.command.toLowerCase().endsWith('.gguf'))

const editServiceUrl = computed(() => {
  const host = (editForm.value.host || 'http://127.0.0.1:8080').replace(/\/+$/, '')
  const base = editForm.value.base_path || ''
  // base_path 仅为 "/" 时不拼到 URL 上，避免显示 http://host:port/
  if (!base || base === '/') return host
  return host + (base.startsWith('/') ? base : `/${base}`)
})

function onEditUrlChange(e: Event) {
  const url = (e.target as HTMLInputElement).value
  try {
    const u = new URL(url)
    const port = parseInt(u.port) || editForm.value.port || 8080
    editForm.value.host = `${u.protocol}//${u.hostname}:${port}`
    editForm.value.port = port
    editForm.value.base_path = u.pathname
  } catch {
    // 解析失败时不做任何更新
  }
}

// 提交编辑
const handleEditService = async () => {
  if (!editForm.value.display_name) return
  try {
    await wsp.editService({
      serviceId: editForm.value.id,
      displayName: editForm.value.display_name,
      description: editForm.value.description || undefined,
      exePath: editForm.value.command,
      port: editForm.value.port,
      autoStart: editForm.value.auto_start,
      category: CAT_NAME_TO_CODE[editForm.value.category_name] || '',
      categoryName: editForm.value.category_name || undefined,
      subCategoryName: editForm.value.sub_category_name || undefined,
      host: editForm.value.host || undefined,
      basePath: editForm.value.base_path || undefined,
    })
    showEditDialog.value = false
  } catch (err) {
    console.error('[WSP] 编辑服务失败:', err)
  }
}

const CAT_LABELS: Record<string, string> = {
  asr: '语音识别', ocr: '文字识别', image: '图像识别',
  video: '视频分析', text: '文本挖掘', translate: '智能翻译',
  document: '文档输出', agent: '智能体', feature: '功能',
}
const CAT_NAME_TO_CODE = Object.fromEntries(
  Object.entries(CAT_LABELS).map(([k, v]) => [v, k])
)

// 级联分类选项：第一级大分类中文名，第二级子分类中文名
const filteredCategoryOptions = computed(() => {
  const items = (menuStore.wspMenuItems || []).filter(m => m.is_visible !== false)
  const grouped: Record<string, { value: string; label: string }[]> = {}
  for (const m of items) {
    const parentName = CAT_LABELS[m.category] || m.category
    if (!grouped[parentName]) grouped[parentName] = []
    grouped[parentName].push({ value: m.title, label: m.title })
  }
  return Object.entries(grouped)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([parentName, children]) => ({
      value: parentName,
      label: parentName,
      children: children.sort((a, b) => a.label.localeCompare(b.label)),
    }))
    .filter(g => g.children.length > 0)
})

// handle add service
const handleAddService = async () => {

  if (!newService.value.display_name || !newService.value.name) return
  try {
    const [exe, ...args] = newService.value.command.split(' ')

    await wsp.registerService({
      name: newService.value.name,
      display_name: newService.value.display_name,
      description: newService.value.description,
      category: CAT_NAME_TO_CODE[newService.value.category_name] || newService.value.category || '',
      category_name: newService.value.category_name,
      sub_category_name: newService.value.sub_category_name,
      port: newService.value.port,
      exe_path: newService.value.command,
      args: args.length > 0 ? JSON.stringify(args) : null,
      auto_st: newService.value.auto_start,
      host: `http://127.0.0.1:${newService.value.port}`,
    })
    showAddDialog.value = false
    newService.value = { name: '', display_name: '', description: '', category: '', category_name: '', sub_category_name: '', category_model: [], port: 8080, command: '', auto_start: false }
  } catch (err) {
    const msg = (err as any)?.message || String(err)
    if (msg.includes('UNIQUE constraint failed') || msg.includes('已存在') || msg.includes('already exists')) {
      ElMessage.warning(t('wsp.serviceNameExists'))
    } else {
      ElMessage.error(t('wsp.addServiceFailed', { msg }))
    }
  }
}

function incrementNewPort() { newService.value.port++ }
function decrementNewPort() { if (newService.value.port > 1) newService.value.port-- }
function incrementEditPort() { editForm.value.port++ }
function decrementEditPort() { if (editForm.value.port > 1) editForm.value.port-- }

// 删除服务
const handleDelete = async (serviceId: string, serviceName: string) => {
  try {
    await ElMessageBox.confirm(
      t('wsp.confirmDelete', { name: serviceName }),
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
    await wsp.removeService(serviceId)
    window.dispatchEvent(new CustomEvent('model-deleted', { detail: { modelId: serviceId } }))
  } catch (err) {
    console.error('Failed to delete service:', err)
    alert(t('wsp.deleteFailed') + ': ' + err)
  }
}

// 使用 Tauri dialog 选择文件
const selectFile = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{
        name: t('wsp.allFiles'),
        extensions: ['*']
      }]
    })

    if (selected && typeof selected === 'string') {
      uploadFilePath.value = selected
      const parts = selected.replace(/\\/g, '/').split('/')
      uploadFileName.value = parts[parts.length - 1] || t('wsp.unknownFile')
      console.log('[WSP] 已选择文件:', uploadFileName.value, uploadFilePath.value)
    }
  } catch (err) {
    console.error('[WSP] 选择文件失败:', err)
  }
}

const handleFileDrop = (e: DragEvent) => {
  e.preventDefault()
  isDragging.value = false
  selectFile()
}

const handleFileSelect = (e: Event) => {
  const target = e.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    const file = target.files[0]
    //uploadFilePath.value = file
    // 在 Tauri 中，File 对象通常包含 path 属性
    uploadFilePath.value = (file as any).path || ''
  }
}

// handle upload service
const handleUpload = async () => {
  if (!uploadFilePath.value) {
    alert(t('wsp.selectFileFirst'));
    return;
  }

  try {
    const filePath = uploadFilePath.value;
    const fileName = filePath.split(/[/\\]/).pop() || t('wsp.unknownFile');
    const baseName = fileName.replace(/\.[^/.]+$/, "");
    const timestamp = Date.now().toString(36);
    const safeName = `${baseName.toLowerCase().replace(/[^a-z0-9]/g, '-')}-${timestamp}` || `wsp-${timestamp}`;

    // 解析 host、base_path 和 port
    let host = ''
    let basePath = ''
    let uploadPort = 0
    if (uploadUrl.value) {
      try {
        const url = new URL(uploadUrl.value);
        host = `${url.protocol}//${url.host}`;
        basePath = url.pathname || '/';
        uploadPort = parseInt(url.port) || 8080
      } catch {
        host = uploadUrl.value;
        basePath = '/';
      }
    } else if (!isGgufUpload.value) {
      uploadPort = 8080 + Math.floor(Math.random() * 1000)
      host = `http://127.0.0.1:${uploadPort}`;
      basePath = '/';
    }

    // 使用用户输入的服务名称，如果为空则使用文件名
    const displayName = uploadDisplayName.value.trim() || baseName;

    // 检查服务名称是否已存在
    if (wsp.services.some(s => s.display_name === displayName)) {
      ElMessage.warning(t('wsp.serviceNameExists'))
      return
    }

    const catArr = uploadCategory.value || []
    await wsp.registerService({
      name: safeName,
      display_name: displayName,
      description: t('wsp.descriptionFile', { name: fileName }),
      category: CAT_NAME_TO_CODE[catArr[0]] || '',
      category_name: catArr[0] || '',
      sub_category_name: catArr[1] || '',
      exe_path: filePath,
      port: uploadPort,
      auto_start: false,
      host: host || undefined,
      base_path: basePath || undefined,
    });
    showUploadDialog.value = false;
    uploadFilePath.value = '';

  } catch (err) {
    const msg = (err as any)?.message || String(err)
    if (msg.includes('UNIQUE constraint failed') || msg.includes('已存在') || msg.includes('already exists')) {
      ElMessage.warning(t('wsp.serviceNameExists'))
    } else {
      ElMessage.error(t('wsp.installFailed') + msg)
    }
  }
}

</script>

<template>
  <div class="wsp-view" :data-theme-id="themeStore.draftPreset.id" style="background: var(--bg-primary);">
    <el-tabs v-model="activeTab" class="wsp-tabs" type="border-card">
      <el-tab-pane :label="$t('wsp.tabServices')" name="wsp">
        <div class="page-header">
          <div class="header-left">
            <h2 style="display: flex; align-items: center;">
              <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                style="margin-right: 8px; flex-shrink: 0;">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                <line x1="8" y1="21" x2="16" y2="21"></line>
                <line x1="12" y1="17" x2="12" y2="21"></line>
              </svg>
              {{ $t('wsp.title') }}
            </h2>
            <div class="header-stats">
              <span class="stat">{{ $t('wsp.totalServices', { n: wsp.services.length }) }}</span>
              <span class="stat running">{{ $t('wsp.runningCount', { n: runningCount }) }}</span>
            </div>
          </div>
          <div class="header-actions">
            <button ref="uploadServiceBtnRef" class="btn btn-primary" @click="showUploadDialog = true">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                <polyline points="17 8 12 3 7 8"></polyline>
                <line x1="12" y1="3" x2="12" y2="15"></line>
              </svg>
              {{ $t('wsp.uploadService') }}
            </button>
            <button ref="addServiceBtnRef" class="btn btn-primary" @click="showAddDialog = true">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                <line x1="12" y1="5" x2="12" y2="19"></line>
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
              {{ $t('wsp.addService') }}
            </button>
            <button ref="startAllBtnRef" class="btn btn-primary" @click="wsp.startAll()">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                <polygon points="5,3 19,12 5,21" />
              </svg>
              {{ $t('wsp.startAll') }}
            </button>
            <button ref="stopAllBtnRef" class="btn btn-danger" @click="wsp.stopAll()">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                <rect x="3" y="3" width="18" height="18" rx="2" />
              </svg>
              {{ $t('wsp.stopAll') }}
            </button>
          </div>
        </div>
        <!-- ── 工具栏：搜索 + 筛选 + 视图切换 ── -->
        <div class="toolbar-wrap">
          <div class="toolbar">
            <div class="toolbar-left">
              <el-input v-model="searchName" :placeholder="$t('wsp.searchPlaceholder')" size="small"
                style="width: 300px;">
                <template #prefix>
                  <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                    <circle cx="11" cy="11" r="8" />
                    <line x1="21" y1="21" x2="16.65" y2="16.65" />
                  </svg>
                </template>
                <template #suffix>
                  <SearchClearButton v-if="searchName" @clear="searchName = ''" />
                </template>
              </el-input>
              <el-select v-model="filterStatus" :placeholder="$t('wsp.filterAllStatus')" clearable size="small"
                style="width: 130px;" popper-class="wsp-status-filter-popper">
                <el-option v-for="opt in statusOptions" :key="opt.value" :value="opt.value" :label="opt.label">
                  <el-tag :type="opt.tagType" size="small" effect="dark" class="wsp-status-option-tag" style="pointer-events:none;">{{ opt.label
                  }}</el-tag>
                </el-option>
              </el-select>
            </div>
            <div class="toolbar-right">
              <button class="view-btn" :class="{ active: viewMode === 'card' }" :title="$t('wsp.cardView')"
                @click="viewMode = 'card'">
                <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
                  <rect x="3" y="3" width="7" height="7" rx="1" />
                  <rect x="14" y="3" width="7" height="7" rx="1" />
                  <rect x="3" y="14" width="7" height="7" rx="1" />
                  <rect x="14" y="14" width="7" height="7" rx="1" />
                </svg>
              </button>
              <button class="view-btn" :class="{ active: viewMode === 'table' }" :title="$t('wsp.tableView')"
                @click="viewMode = 'table'">
                <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
                  <line x1="3" y1="6" x2="21" y2="6" />
                  <line x1="3" y1="12" x2="21" y2="12" />
                  <line x1="3" y1="18" x2="21" y2="18" />
                </svg>
              </button>
            </div>
          </div>
        </div><!-- /toolbar-wrap -->

        <div
          ref="serviceBodyRef"
          class="service-body"
          :class="{ 'service-body--card': viewMode === 'card' || filteredServices.length === 0 }"
        >
          <!-- 卡片/列表共用空状态：同一 DOM，切换视图不位移 -->
          <div v-if="filteredServices.length === 0" class="empty-state">
            <div class="empty-icon">
              <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                style="margin-right: 8px; flex-shrink: 0;">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                <line x1="8" y1="21" x2="16" y2="21"></line>
                <line x1="12" y1="17" x2="12" y2="21"></line>
              </svg>
            </div>
            <div class="empty-title">{{ wsp.services.length === 0 ? $t('wsp.noServices') : $t('wsp.noMatchService') }}
            </div>
            <div class="empty-desc">{{ wsp.services.length === 0 ? $t('wsp.noServicesDesc') : $t('wsp.noMatchDesc') }}
            </div>
            <div v-if="wsp.services.length === 0" class="empty-actions">
              <button ref="addServiceEmptyBtnRef" class="btn btn-primary" @click="showAddDialog = true">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <line x1="12" y1="5" x2="12" y2="19" />
                  <line x1="5" y1="12" x2="19" y2="12" />
                </svg>
                {{ $t('wsp.addService') }}
              </button>
              <button ref="uploadServiceEmptyBtnRef" class="btn btn-primary" @click="showUploadDialog = true">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="17 8 12 3 7 8" />
                  <line x1="12" y1="3" x2="12" y2="15" />
                </svg>
                {{ $t('wsp.uploadPackage') }}
              </button>
            </div>
          </div>

          <!-- ── 卡片视图 ── -->
          <div v-else-if="viewMode === 'card'" class="service-grid">
              <div v-for="svc in filteredServices" :key="svc.id" class="service-card">
                <div class="card-header">
                  <div class="status-dot" :style="{ background: statusColor(svc.runtime_status) }"></div>
                  <div class="card-info">
                    <h3>{{ svc.display_name }}</h3>
                    <span class="card-meta">{{ svc.sub_category_name || svc.category_name || $t('wsp.cat_' +
                      svc.category)
                    }}<template v-if="svc.port"> · {{ $t('wsp.port', { port: svc.port }) }}</template></span>
                  </div>
                  <el-tag :type="statusTagType(svc.runtime_status)" size="small" effect="light"
                    :disable-transitions="true">
                    {{ statusLabel(svc.runtime_status) }}
                  </el-tag>
                </div>
                <div v-if="svc.runtime_error" class="card-desc" style="color: var(--color-danger);">⚠️ {{
                  svc.runtime_error }}</div>
                <div v-else-if="svc.description" class="card-desc">{{ svc.description }}</div>
                <div v-else class="card-desc" style="opacity: 0.5;">{{ $t('wsp.noDescription') }}</div>
                <div class="card-actions">
                  <button v-if="svc.runtime_status !== 'running'" class="btn-start" :disabled="!!startingIds[svc.id]"
                    @click="handleStart(svc.id)">
                    <svg v-if="!startingIds[svc.id]" viewBox="0 0 24 24" width="13" height="13" fill="currentColor">
                      <polygon points="5,3 19,12 5,21" />
                    </svg>
                    <svg v-else viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor"
                      stroke-width="2" class="spin-icon">
                      <path d="M12 2a10 10 0 1 0 10 10" stroke-linecap="round" />
                    </svg>
                    {{ startingIds[svc.id] ? $t('wsp.starting') : $t('wsp.start') }}
                  </button>
                  <button v-if="svc.runtime_status === 'running'" class="btn-restart"
                    @click="wsp.restartService(svc.id)">
                    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                      <path d="M3 3v5h5" />
                    </svg>
                    {{ $t('wsp.restart') }}
                  </button>
                  <button v-if="svc.runtime_status === 'running'" class="btn-stop" @click="wsp.stopService(svc.id)">
                    <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor">
                      <rect x="3" y="3" width="18" height="18" rx="2" />
                    </svg>
                    {{ $t('wsp.stop') }}
                  </button>
                  <button v-if="svc.runtime_status !== 'running'" class="btn-edit" @click="openEditDialog(svc)">
                    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                    </svg>
                    {{ $t('wsp.edit') }}
                  </button>
                  <button class="btn-delete" @click="handleDelete(svc.id, svc.display_name)">
                    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
                      <polyline points="3 6 5 6 21 6" />
                      <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                      <path d="M10 11v6M14 11v6" />
                      <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
                    </svg>
                    {{ $t('wsp.delete') }}
                  </button>
                </div>
              </div>
          </div>

          <!-- ── 列表视图：表头固定，仅表体滚动（与全部工具一致） ── -->
          <div v-else class="table-card wsp-table-card">
                <div ref="wspTableHeadRef" class="wsp-table-head">
                  <table class="wsp-native-table">
                    <colgroup>
                      <col class="col-status" />
                      <col class="col-name" />
                      <col class="col-category" />
                      <col class="col-port" />
                      <col class="col-pid" />
                      <col class="col-desc" />
                      <col class="col-actions" />
                    </colgroup>
                    <thead>
                      <tr>
                        <th class="is-center">{{ $t('wsp.colStatus') }}</th>
                        <th>{{ $t('wsp.colName') }}</th>
                        <th>{{ $t('wsp.colCategory') }}</th>
                        <th class="is-center">{{ $t('wsp.colPort') }}</th>
                        <th class="is-center">{{ $t('wsp.colPid') }}</th>
                        <th>{{ $t('wsp.colDescription') }}</th>
                        <th class="is-right">{{ $t('wsp.colActions') }}</th>
                      </tr>
                    </thead>
                  </table>
                </div>
                <div ref="wspTableBodyRef" class="wsp-table-body-scroll" @scroll="onWspTableBodyScroll">
                  <table class="wsp-native-table">
                    <colgroup>
                      <col class="col-status" />
                      <col class="col-name" />
                      <col class="col-category" />
                      <col class="col-port" />
                      <col class="col-pid" />
                      <col class="col-desc" />
                      <col class="col-actions" />
                    </colgroup>
                    <tbody>
                      <tr
                        v-for="row in filteredServices"
                        :key="row.id"
                        :class="{ 'row-error': !!row.runtime_error }"
                      >
                        <td class="is-center">
                          <el-tag :type="statusTagType(row.runtime_status)" size="small" effect="light"
                            :disable-transitions="true">
                            {{ statusLabel(row.runtime_status) }}
                          </el-tag>
                        </td>
                        <td>
                          <div class="tbl-name" :title="row.display_name">
                            <span>{{ row.display_name }}</span>
                            <span v-if="row.runtime_error" class="tbl-error-tip" :title="row.runtime_error">⚠️</span>
                          </div>
                        </td>
                        <td :title="row.sub_category_name || row.category_name || $t('wsp.cat_' + row.category)">
                          {{ row.sub_category_name || row.category_name || $t('wsp.cat_' + row.category) }}
                        </td>
                        <td class="is-center">
                          <span class="tbl-port">{{ row.port }}</span>
                        </td>
                        <td class="is-center">
                          <span class="tbl-pid">{{ row.pid ?? '—' }}</span>
                        </td>
                        <td>
                          <span class="tbl-desc" :title="row.description || ''">{{ row.description || '—' }}</span>
                        </td>
                        <td class="is-right">
                          <div class="tbl-actions">
                            <button v-if="row.runtime_status !== 'running'" class="btn-start" :disabled="!!startingIds[row.id]"
                              @click="handleStart(row.id)">
                              <svg v-if="!startingIds[row.id]" viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                <polygon points="5,3 19,12 5,21" />
                              </svg>
                              <svg v-else viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
                                stroke-width="2" class="spin-icon">
                                <path d="M12 2a10 10 0 1 0 10 10" stroke-linecap="round" />
                              </svg>
                              {{ startingIds[row.id] ? $t('wsp.starting') : $t('wsp.start') }}
                            </button>
                            <button v-if="row.runtime_status === 'running'" class="btn-restart"
                              @click="wsp.restartService(row.id)">
                              <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
                                stroke-width="2">
                                <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                                <path d="M3 3v5h5" />
                              </svg>
                              {{ $t('wsp.restart') }}
                            </button>
                            <button v-if="row.runtime_status === 'running'" class="btn-stop" @click="wsp.stopService(row.id)">
                              <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                                <rect x="3" y="3" width="18" height="18" rx="2" />
                              </svg>
                              {{ $t('wsp.stop') }}
                            </button>
                            <button v-if="row.runtime_status !== 'running'" class="btn-edit" @click="openEditDialog(row)">
                              <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
                                stroke-width="2">
                                <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                                <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                              </svg>
                              {{ $t('wsp.edit') }}
                            </button>
                            <button class="btn-delete" @click="handleDelete(row.id, row.display_name)">
                              <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
                                stroke-width="2">
                                <polyline points="3 6 5 6 21 6" />
                                <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                                <path d="M10 11v6M14 11v6" />
                                <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" />
                              </svg>
                              {{ $t('wsp.delete') }}
                            </button>
                          </div>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
          </div><!-- /table-card -->
        </div><!-- /service-body -->

        <!-- 添加服务弹窗 -->
        <div v-if="showAddDialog" class="overlay">
          <div class="dialog add-service-dialog">
            <div class="dialog-header">
              <h3>{{ $t('wsp.addWspService') }}</h3>
              <button class="dialog-close" @click="showAddDialog = false">
                <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            <div class="form-group">
              <label>
                <span style="color: red;">*</span>
                {{ $t('wsp.serviceName') }}</label>
              <input v-model="newService.display_name" :placeholder="$t('wsp.serviceNamePlaceholder')" />
            </div>
            <div class="form-group">
              <label>{{ $t('wsp.serviceId') }}</label>
              <input v-model="newService.name" :placeholder="$t('wsp.serviceIdPlaceholder')" />
            </div>
            <div class="form-row">
              <div class="form-group">
                <label>
                  <span style="color: red;">*</span>
                  {{ $t('wsp.category') }}</label>
                <el-cascader v-model="newService.category_model" :options="filteredCategoryOptions"
                  :placeholder="t('wsp.selectType')" popper-class="category-cascader-popper" :teleported="true"
                  clearable filterable class="category-cascader"
                  @change="(val) => { newService.category_name = (val && val[0]) || ''; newService.sub_category_name = (val && val[1]) || ''; }" />
              </div>
              <div class="form-group">
                <label>
                  <span style="color: red;">*</span>
                  {{ $t('wsp.portLabel') }}</label>
                <div class="number-input-wrap">
                  <input v-model.number="newService.port" type="number" />
                  <div class="number-arrows">
                    <button class="number-arrow number-arrow-up" type="button" @click="incrementNewPort">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="18 15 12 9 6 15"></polyline>
                      </svg>
                    </button>
                    <button class="number-arrow number-arrow-down" type="button" @click="decrementNewPort">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="6 9 12 15 18 9"></polyline>
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </div>
            <div class="form-group">
              <label>
                <span style="color: red;">*</span>
                {{ $t('wsp.startCommand') }}</label>
              <input v-model="newService.command" :placeholder="$t('wsp.startCommandPlaceholder')" />
            </div>
            <div class="form-group">
              <label>{{ $t('wsp.description') }}</label>
              <textarea v-model="newService.description" :placeholder="$t('wsp.descriptionPlaceholder')" rows="3"
                style="height: 80px;"></textarea>
            </div>
            <div class="form-check">
              <input type="checkbox" v-model="newService.auto_start" id="autoStart" />
              <label for="autoStart">{{ $t('wsp.autoStart') }}</label>
            </div>
            <div class="dialog-actions">
              <button class="btn btn-cancel" @click="showAddDialog = false">{{ $t('common.cancel') }}</button>
              <button class="btn btn-primary" :disabled="!canAddService" @click="handleAddService">{{
                $t('wsp.addService')
              }}</button>
            </div>
          </div>
        </div>

        <!-- 编辑服务弹窗 -->
        <div v-if="showEditDialog" class="overlay">
          <div class="dialog">
            <div class="dialog-header">
              <h3>{{ $t('wsp.editWspService') }}</h3>
              <button class="dialog-close" @click="showEditDialog = false">
                <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            <div class="form-group">
              <label>{{ $t('wsp.serviceName') }}</label>
              <input v-model="editForm.display_name" :placeholder="$t('wsp.serviceNamePlaceholder')" />
            </div>
            <div class="form-row">
              <div class="form-group">
                <label>{{ $t('wsp.category') }}</label>
                <el-cascader v-model="editForm.category_model" :options="filteredCategoryOptions"
                  :placeholder="t('wsp.selectType')" popper-class="category-cascader-popper" :teleported="true"
                  clearable filterable class="category-cascader" @change="onEditCategoryChange" />
              </div>
              <div class="form-group">
                <label>{{ $t('wsp.portLabel') }}</label>
                <div class="number-input-wrap">
                  <input v-model.number="editForm.port" type="number" />
                  <div class="number-arrows">
                    <button class="number-arrow number-arrow-up" type="button" @click="incrementEditPort">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="18 15 12 9 6 15"></polyline>
                      </svg>
                    </button>
                    <button class="number-arrow number-arrow-down" type="button" @click="decrementEditPort">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="6 9 12 15 18 9"></polyline>
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </div>
            <div class="form-group" v-if="isEditExeService && !isEditGgufService">
              <label>{{ $t('wsp.serviceUrl') }}</label>
              <input :value="editServiceUrl" @input="onEditUrlChange($event)"
                :placeholder="$t('wsp.serviceUrlPlaceholder')" />
            </div>
            <div class="form-group" v-else-if="!isEditGgufService">
              <label>{{ $t('wsp.startCommand') }}</label>
              <input v-model="editForm.command" :placeholder="$t('wsp.startCommandPlaceholder')" />
            </div>
            <div class="form-group">
              <label>{{ $t('wsp.description') }}</label>
              <textarea v-model="editForm.description" :placeholder="$t('wsp.descriptionPlaceholder')" rows="3"
                style="height: 80px;"></textarea>
            </div>
            <div class="form-check">
              <input type="checkbox" v-model="editForm.auto_start" id="editAutoStart" />
              <label for="editAutoStart">{{ $t('wsp.autoStart') }}</label>
            </div>
            <div class="dialog-actions">
              <button class="btn btn-cancel" @click="showEditDialog = false">{{ $t('common.cancel') }}</button>
              <button class="btn btn-primary" @click="handleEditService">{{ $t('common.save') }}</button>
            </div>
          </div>
        </div>

        <!-- 上传服务弹窗 -->
        <div v-if="showUploadDialog" class="overlay">
          <div class="dialog upload-service-dialog">
            <div class="dialog-header">
              <h3>{{ $t('wsp.uploadWspPackage') }}</h3>
              <button class="dialog-close"
                @click="showUploadDialog = false; uploadFileName = ''; uploadFilePath = ''; uploadDisplayName = ''">
                <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            <div class="upload-zone" :class="{ dragging: isDragging, 'has-file': uploadFileName }"
              @dragover.prevent="isDragging = true" @dragleave="isDragging = false" @drop.prevent="handleFileDrop"
              @click="selectFile()">
              <template v-if="uploadFileName">
                <div class="file-info">
                  <span class="file-icon">📦</span>
                  <span class="file-name">{{ uploadFileName }}</span>
                  <span class="file-path">{{ uploadFilePath }}</span>
                </div>
              </template>
              <template v-else>
                <svg viewBox="0 0 24 24" width="36" height="36" stroke="currentColor" stroke-width="1.5" fill="none"
                  style="color: var(--text-secondary);">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                  <polyline points="17 8 12 3 7 8"></polyline>
                  <line x1="12" y1="3" x2="12" y2="15"></line>
                </svg>
                <p class="upload-hint">{{ $t('wsp.clickOrDrag') }}</p>
                <p class="upload-formats">{{ $t('wsp.exeOnly') }}</p>
              </template>
            </div>
            <div class="form-group" style="margin-top: 16px;">
              <label>
                <span style="color: red;">*</span>
                {{ $t('wsp.serviceName') }}</label>
              <input v-model="uploadDisplayName" :placeholder="$t('wsp.serviceNamePlaceholder')" />
            </div>
            <div class="form-group">
              <label>
                <span style="color: red;">*</span>
                {{ $t('wsp.category') }}</label>
              <el-cascader v-model="uploadCategory" :options="filteredCategoryOptions"
                :placeholder="t('wsp.selectType')" popper-class="category-cascader-popper" :teleported="true" clearable
                filterable class="category-cascader" />
            </div>
            <div class="form-group">
              <label>
                <span v-if="!isGgufUpload" style="color: red;">*</span>
                {{ $t('wsp.serviceUrl') }}
                <span v-if="isGgufUpload" style="color: #999; font-weight: normal;">({{ $t('wsp.optional') }})</span>
              </label>
              <input v-model="uploadUrl" :placeholder="$t('wsp.serviceUrlPlaceholder')" />
            </div>
            <div class="dialog-actions">
              <button class="btn btn-cancel"
                @click="showUploadDialog = false; uploadFileName = ''; uploadFilePath = ''; uploadDisplayName = ''">{{
                  $t('common.cancel') }}</button>
              <button class="btn btn-primary" :disabled="!canUpload" @click="handleUpload">{{
                $t('wsp.uploadAndInstall')
              }}</button>
            </div>
          </div>
        </div>

        <!-- 错误提示 Toast -->
        <transition name="toast-fade">
          <div v-if="showErrorToast" class="error-toast">
            <div class="error-toast-icon">
              <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="12" y1="8" x2="12" y2="12"></line>
                <line x1="12" y1="16" x2="12.01" y2="16"></line>
              </svg>
            </div>
            <span class="error-toast-message">{{ errorToastMessage }}</span>
          </div>
        </transition>

        <!-- 首次使用引导 -->
        <Teleport to="body">
          <div v-if="showGuide" class="wsp-guide-overlay">
            <div class="wsp-guide-mask"></div>
            <div v-if="guideTargetRect" class="wsp-guide-highlight" :style="{
              top: guideTargetRect.top - 4 + 'px',
              left: guideTargetRect.left - 4 + 'px',
              width: guideTargetRect.width + 8 + 'px',
              height: guideTargetRect.height + 8 + 'px',
            }"></div>
            <div v-if="guideTargetRect" class="wsp-guide-tooltip" :style="tooltipStyle">
              <div class="wsp-guide-arrow" :style="tooltipArrowStyle"></div>
              <div class="wsp-guide-step">{{ currentGuideStep + 1 }} / {{ guideSteps.length }}</div>
              <div class="wsp-guide-title">{{ guideSteps[currentGuideStep].title }}</div>
              <div class="wsp-guide-content">{{ guideSteps[currentGuideStep].content }}</div>
              <div class="wsp-guide-actions">
                <button v-if="currentGuideStep < guideSteps.length - 1" class="wsp-guide-btn wsp-guide-btn-skip"
                  @click="closeGuide">{{ $t('wsp.guideSkip') }}</button>
                <button v-if="currentGuideStep > 0" class="wsp-guide-btn wsp-guide-btn-prev" @click="prevGuideStep">{{
                  $t('wsp.guidePrev') }}</button>
                <button v-if="currentGuideStep < guideSteps.length - 1" class="wsp-guide-btn wsp-guide-btn-next"
                  @click="nextGuideStep">{{ $t('wsp.guideNext') }}</button>
                <button v-else class="wsp-guide-btn wsp-guide-btn-finish" @click="closeGuide">{{ $t('wsp.guideFinish')
                }}</button>
              </div>
            </div>
          </div>
        </Teleport>
      </el-tab-pane>

      <!-- Tab 2: 依赖服务 -->
      <el-tab-pane :label="$t('wsp.tabDependencies')" name="deps">
        <div class="dep-services-wrap">
          <div class="dep-header">
            <span class="dep-subtitle">{{ $t('wsp.depSubtitle') }}</span>
            <div class="dep-header-actions">
              <el-button class="btn btn-primary" type="primary" size="small" @click="startAllDepServices"
                :loading="depStartingAll" :disabled="depBatchBusy || depIndividualBusy">
                <svg v-if="!depStartingAll" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                  <polygon points="5,3 19,12 5,21" />
                </svg>
                {{ $t('wsp.startAll') }}
              </el-button>
              <el-button class="btn btn-danger" type="danger" size="small" @click="stopAllDepServices"
                :loading="depStoppingAll" :disabled="depBatchBusy || depIndividualBusy">
                <svg v-if="!depStoppingAll" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                  <rect x="4" y="4" width="16" height="16" rx="2" />
                </svg>
                {{ $t('wsp.stopAll') }}
              </el-button>
              <el-switch :model-value="depRecovery.fastkg?.global_enabled ?? true"
                :loading="globalRecoverySaving" :disabled="globalRecoverySaving"
                :aria-busy="globalRecoverySaving" active-text="全局自动恢复"
                aria-label="全局自动恢复" @change="saveGlobalRecovery" />
            </div>
          </div>
          <div class="dep-services-grid">
            <div v-for="svc in depServices" :key="svc.id" class="dep-service-card"
              :class="{ running: svc.running, 'not-installed': !svc.installed }">
              <el-tooltip v-if="!svc.installed"
                :content="$t('wsp.depNotInstalled')"
                placement="top">
                <div class="svc-header">
                  <span class="svc-status-dot" :class="{ running: svc.running }"></span>
                  <span class="svc-name">{{ svc.name }}</span>
                  <el-tag :type="getDepServiceTagType(svc)" size="small">
                    {{ getDepServiceStatusLabel(svc) }}
                  </el-tag>
                </div>
              </el-tooltip>
              <div v-else class="svc-header">
                <span class="svc-status-dot" :class="{ running: svc.running }"></span>
                <span class="svc-name">{{ svc.name }}</span>
                <el-tag :type="getDepServiceTagType(svc)" size="small">
                  {{ getDepServiceStatusLabel(svc) }}
                </el-tag>
              </div>
              <div class="svc-meta">
                <span>{{ svc.host }}:{{ svc.port }}</span>
                <span v-if="svc.using_fallback" class="dep-port-fallback">{{ getDepPortFallbackText(svc) }}</span>
                <span v-if="svc.pid">PID: {{ svc.pid }}</span>
                <span v-else class="no-pid">PID: --</span>
                <span v-if="svc.starter_app">启动方: {{ svc.starter_app }}</span>
                <span v-if="svc.lease_count">消费者: {{ svc.consumers?.join(', ') || svc.lease_count }}</span>
              </div>
              <div class="svc-desc">{{ svc.description }}</div>
              <div v-if="svc.installed && recoveryText(svc.id)" class="svc-desc" role="status">{{ recoveryText(svc.id) }}</div>
              <div v-if="svc.installed && !svc.running && !['failed', 'paused', 'idle'].includes(depRecovery[svc.id]?.recovery_state || '') && depRecovery[svc.id]?.desired_state !== 'stopped' && depPortStates[svc.id] && ['probing', 'occupied', 'configuring', 'starting', 'retrying'].includes(depPortStates[svc.id].phase)"
                class="svc-desc dep-port-attempt" role="status">
                {{ getDepPortStatusText(svc, depPortStates[svc.id]) }}
              </div>
              <div v-else-if="svc.installed && svc.port_occupied" class="svc-desc dep-port-conflict" role="status">
                端口 {{ svc.port }} 已被占用，启动时自动选择可用端口
              </div>
              <div v-if="depRecovery[svc.id]" class="svc-meta svc-recovery-settings">
                <el-checkbox :model-value="depRecovery[svc.id].auto_start" @change="value => saveRecoverySetting(svc.id, 'autoStart', value)">随应用启动</el-checkbox>
                <el-checkbox :model-value="depRecovery[svc.id].service_enabled" :disabled="globalRecoverySaving || !depRecovery[svc.id].global_enabled" @change="value => saveRecoverySetting(svc.id, 'enabled', value)">自动恢复</el-checkbox>
              </div>
              <div class="svc-actions">
                <button v-if="svc.installed && depRecovery[svc.id]?.can_retry" class="btn-start" :disabled="depBatchBusy" @click="recoveryAction(svc.id, 'retry')">立即重试</button>
                <button v-if="['queued', 'starting', 'backoff', 'waiting_dependency', 'waiting_shared_owner'].includes(depRecovery[svc.id]?.recovery_state || '')"
                  class="btn-stop" :disabled="depBatchBusy" @click="recoveryAction(svc.id, 'pause')">停止自动恢复</button>
                <button v-if="svc.id === 'fastkg' && !svc.installed" class="btn-update btn-install-kg"
                  @click="fastKgInstallDialogVisible = true">
                  <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                    <polyline points="7 10 12 15 17 10" />
                    <line x1="12" y1="15" x2="12" y2="3" />
                  </svg>
                  {{ $t('wsp.fastkgInstallBtn') }}
                </button>
                <button v-else-if="svc.can_stop || (svc.ownership === 'shared_managed' && svc.lease_registered)" class="btn-stop"
                  :disabled="depBatchBusy || depStoppingIds.has(svc.id) || (svc.ownership !== 'shared_managed' && hasOtherKgConsumers(svc))"
                  @click="stopDep(svc.id)">
                  <svg v-if="!depStoppingIds.has(svc.id)" viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                    <rect x="3" y="3" width="18" height="18" rx="2" />
                  </svg>
                  <svg v-else viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
                    stroke-width="2" class="spin-icon">
                    <path d="M12 2a10 10 0 1 0 10 10" stroke-linecap="round" />
                  </svg>
                  {{ depStoppingIds.has(svc.id) ? $t('wsp.stopping') : svc.ownership === 'shared_managed' ? '退出共享使用' : $t('wsp.stop') }}
                </button>
                <button v-else-if="svc.can_start" class="btn-start"
                  :disabled="depBatchBusy || depStartingIds.has(svc.id) || depStoppingIds.has(svc.id) || (!svc.installed && svc.id !== 'fastplan')"
                  @click="startDep(svc.id)">
                  <svg v-if="!depStartingIds.has(svc.id)" viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                    <polygon points="5,3 19,12 5,21" />
                  </svg>
                  <svg v-else viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
                    stroke-width="2" class="spin-icon">
                    <path d="M12 2a10 10 0 1 0 10 10" stroke-linecap="round" />
                  </svg>
                  {{ depStartingIds.has(svc.id) ? $t('wsp.starting') : $t('wsp.start') }}
                </button>
                <button v-if="svc.configurable && svc.installed" class="btn-edit" :disabled="depBatchBusy" @click="openDepEditDialog(svc)">
                  <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                  </svg>
                  {{ $t('wsp.editConfig') }}
                </button>
                <button
                  v-if="svc.id === 'fastkg' && svc.installed && showFastKgOnlineUpdateBtn"
                  class="btn-update"
                  :disabled="fastKgUpdateBusy"
                  @click="openFastKgOnlineUpdate"
                >
                  <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="23 4 23 10 17 10" />
                    <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
                  </svg>
                  {{ fastKgUpdateBusy ? $t('wsp.fastkgUpdating') : $t('wsp.fastkgUpdateBtn') }}
                </button>
                <button
                  v-else-if="svc.id === 'fastkg' && svc.installed && showFastKgOfflineUploadBtn"
                  class="btn-update"
                  :disabled="fastKgUpdateBusy"
                  @click="openFastKgOfflineUpload"
                >
                  <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                    <polyline points="17 8 12 3 7 8" />
                    <line x1="12" y1="3" x2="12" y2="15" />
                  </svg>
                  {{ fastKgUpdateBusy ? $t('wsp.fastkgUpdating') : $t('wsp.fastkgUploadUpdateBtn') }}
                </button>
                <button class="btn-stop" @click="showRecoveryHistory(svc.id)">恢复记录</button>
              </div>
            </div>
          </div>
        </div>
      </el-tab-pane>
    </el-tabs>

    <FastKgInstallDialog v-model="fastKgInstallDialogVisible" @installed="handleFastKgInstalled" />

    <el-dialog
      v-model="recoveryHistoryOpen"
      title="依赖服务恢复记录（最近 50 条）"
      width="720px"
      align-center
      class="dep-edit-dialog"
      modal-class="dep-edit-dialog-overlay"
      :close-on-click-modal="false"
    >
      <el-table :data="recoveryHistory" max-height="420" empty-text="暂无恢复记录">
        <el-table-column prop="created_at" label="时间（UTC）" width="175" />
        <el-table-column prop="phase" label="状态" width="150" />
        <el-table-column prop="attempt" label="次数" width="65" />
        <el-table-column prop="detail" label="说明" />
      </el-table>
      <details v-if="recoveryDiagnostics.length"><summary>当前会话最近进程输出</summary><pre style="white-space: pre-wrap; max-height: 240px; overflow: auto">{{ recoveryDiagnostics.join('\n') }}</pre></details>
    </el-dialog>

    <!-- 编辑依赖服务配置弹窗 -->
    <el-dialog
      v-model="depEditDialogVisible"
      :title="$t('wsp.editConfigTitle')"
      width="400px"
      align-center
      class="dep-edit-dialog"
      modal-class="dep-edit-dialog-overlay"
      :close-on-click-modal="false"
    >
      <div class="dep-edit-form">
        <div class="dep-edit-item">
          <label>{{ $t('wsp.host') }}</label>
          <el-input v-model="depEditForm.host" placeholder="127.0.0.1" />
        </div>
        <div class="dep-edit-item">
          <label>{{ $t('wsp.port') }}</label>
          <el-input-number v-model="depEditForm.port" :min="1" :max="65535" style="width:100%" />
        </div>
        <p class="dep-edit-hint">{{ $t('wsp.configWillRestart') }}</p>
      </div>
      <template #footer>
        <el-button class="btn-cancel" @click="depEditDialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" class="btn-confirm" @click="saveDepConfig">{{ $t('common.save') }}</el-button>
      </template>
    </el-dialog>

    <!-- FastKG 版本更新 / 离线上传更新 -->
    <el-dialog
      v-model="fastKgUpdateDialogVisible"
      :title="fastKgUpdateDialogMode === 'online' ? $t('wsp.fastkgUpdateTitle') : $t('wsp.fastkgUploadUpdateTitle')"
      width="420px"
      align-center
      class="dep-edit-dialog"
      modal-class="dep-edit-dialog-overlay"
      :close-on-click-modal="false"
      :close-on-press-escape="!fastKgUploading"
      :show-close="!fastKgUploading"
    >
      <div v-if="fastKgUploading" class="fastkg-update-progress">
        <el-progress :percentage="fastKgUploadProgress" />
        <p class="dep-edit-hint">{{ fastKgUploadMessage || $t('wsp.fastkgUploading') }}</p>
      </div>
      <div v-else class="dep-edit-form">
        <div class="fastkg-version-row">
          <span>{{ $t('wsp.fastkgUpdateLocal') }}</span>
          <strong>{{ fastKgLocalVersion || $t('wsp.fastkgUnknownVersion') }}</strong>
        </div>
        <div v-if="fastKgUpdateDialogMode === 'online'" class="fastkg-version-row">
          <span>{{ $t('wsp.fastkgUpdateRemote') }}</span>
          <strong class="is-latest">{{ fastKgRemoteVersion || $t('wsp.fastkgUnknownVersion') }}</strong>
        </div>
        <p class="dep-edit-hint">
          {{
            fastKgUpdateDialogMode === 'online'
              ? $t('wsp.fastkgUpdateHint')
              : $t('wsp.fastkgUploadUpdateHint')
          }}
        </p>
      </div>
      <template #footer v-if="!fastKgUploading">
        <el-button class="btn-cancel" @click="fastKgUpdateDialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button
          v-if="fastKgUpdateDialogMode === 'online'"
          type="primary"
          class="btn-confirm"
          :disabled="fastKgUpdateBusy"
          @click="confirmFastKgOnlineUpdate"
        >
          {{ fastKgUpdateBusy ? $t('wsp.fastkgUpdating') : $t('wsp.fastkgUpdateConfirm') }}
        </el-button>
        <el-button
          v-else
          type="primary"
          class="btn-confirm"
          :disabled="fastKgUpdateBusy"
          @click="confirmFastKgOfflineUpload"
        >
          {{ $t('wsp.fastkgSelectZipAndUpdate') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style lang="scss" scoped>
.wsp-view {
  padding: 25px 20px 0px 20px;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.el-tabs--border-card {
  background: transparent;
  border: none;
}

// ── el-tabs 高度约束链：让内容区能正确滚动 ──
.wsp-tabs {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;

  :deep(.el-tabs__header) {
    flex-shrink: 0;
  }

  :deep(.el-tabs__content) {
    flex: 1;
    overflow: hidden;
  }

  :deep(.el-tab-pane) {
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    margin: -10px;

  }

  :deep(.el-tabs__item.is-active) {
    color: #fff !important;
    background: var(--color-primary) !important;
    border-color: var(--color-primary) !important;
  }

  :deep(.el-tabs__item.is-active:hover) {
    color: #fff !important;
  }
}

.toolbar-wrap {
  // border: 1px solid var(--border-color);
  // border-radius: 10px;
  // padding: 8px 12px;
   margin-bottom: 10px;
  // background: var(--bg-secondary, rgba(0, 0, 0, 0.02));
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 8px;
   // border: 1px solid var(--chat-divider-color) !important;
    background: transparent !important;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 4px;
  }
}

.view-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: 1px solid var(--chat-divider-color);
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: var(--bg-hover, rgba(0, 0, 0, 0.06));
    color: var(--text-primary);
  }

  &.active {
    background: var(--color-primary);
    border-color: var(--color-primary);
    color: #fff;
  }
}

.tbl-name {
  display: flex;
  align-items: center;
  gap: 5px;
}

.tbl-error-tip {
  font-size: 13px;
  cursor: help;
}

.tbl-pid {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.tbl-port {
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.tbl-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
  flex-wrap: nowrap;
  min-width: 0;
}

.wsp-native-table .tbl-actions .btn-start,
.wsp-native-table .tbl-actions .btn-restart,
.wsp-native-table .tbl-actions .btn-stop,
.wsp-native-table .tbl-actions .btn-edit,
.wsp-native-table .tbl-actions .btn-delete {
  flex-shrink: 0;
  white-space: nowrap;
}

.service-body {
  flex: 1;
  overflow: hidden;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding-right: 0;

  > template {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

}

.service-body--card {
  overflow-y: auto;
  padding-right: 4px;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;

  &::-webkit-scrollbar { width: 6px; }
  &::-webkit-scrollbar-track { background: transparent; }
  &::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15));
    border-radius: 3px;
  }
}

.wsp-table-card {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: transparent;
}

.wsp-table-head {
  flex-shrink: 0;
  background: var(--table-header-bg-color);
  overflow: hidden; /* 竖向滚动条只在表体，不顶到表头 */
  padding-right: 6px; /* 与表体竖向滚动条占位对齐 */
  box-sizing: border-box;
}

.wsp-table-body-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto; /* 横向：仅内容放不下时出现；竖向：仅表体 */
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;
}

.wsp-table-body-scroll::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.wsp-table-body-scroll::-webkit-scrollbar-track {
  background: transparent;
}

.wsp-table-body-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15));
  border-radius: 3px;
}

.wsp-table-body-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--scrollbar-thumb-hover, rgba(0, 0, 0, 0.25));
}

.wsp-native-table {
  width: 100%;
  min-width: 880px; /* 再窄才出横向滚动条，正常缩窗时列宽等比变窄 */
  border-collapse: collapse;
  table-layout: fixed;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  background: transparent;
}

.wsp-native-table .col-status { width: 9%; }
.wsp-native-table .col-name { width: 15%; }
.wsp-native-table .col-category { width: 11%; }
.wsp-native-table .col-port { width: 8%; }
.wsp-native-table .col-pid { width: 7%; }
.wsp-native-table .col-desc { width: 22%; }
.wsp-native-table .col-actions { width: 28%; }

.wsp-native-table thead th {
  background: var(--table-header-bg-color) !important;
  font-weight: 600;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  padding: 10px 12px;
  border: none;
  white-space: nowrap;
  text-align: left;
}

.wsp-native-table thead th.is-center,
.wsp-native-table tbody td.is-center {
  text-align: center;
}

.wsp-native-table thead th.is-right,
.wsp-native-table tbody td.is-right {
  text-align: right;
}

.wsp-native-table tbody td {
  padding: 10px 12px;
  border: none;
  color: var(--text-primary);
  background-color: transparent;
  vertical-align: middle;
  white-space: nowrap;
}

.wsp-native-table tbody tr:nth-child(even) td {
  background-color: var(--card-background-tmd);
}

.wsp-native-table tbody tr:hover td {
  background-color: var(--dropdown-hover-color);
}

.wsp-native-table tbody tr:nth-child(even):hover td {
  background-color: var(--dropdown-hover-color);
}

.wsp-native-table .tbl-name,
.wsp-native-table .tbl-desc {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.wsp-native-table .tbl-name {
  display: flex;
  align-items: center;
  gap: 5px;
}

.wsp-native-table .tbl-name > span:first-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.page-header {
  container-type: inline-size;
  display: flex;
  align-items: flex-start;
  flex-wrap: wrap;
  gap: 12px;
  margin-bottom: 11px;
  margin-top: 5px;

  .header-left {
    flex: 1 1 auto;
    min-width: 0;

    h2 {
      font-size: var(--font-size-lg);
      font-weight: 700;
      margin-bottom: 2px;
    }                                                                                                        

    .header-stats {
      display: flex;
      gap: 12px;
      font-size: var(--font-size-md);
      color: var(--text-secondary);
    }

    .running {
      // color: var(--color-success);
      color: var(--text-secondary);
    }
  }

  .header-actions {
    flex: 1 1 480px;
    margin-left: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    min-width: 0;
    max-width: 100%;

    .btn {
      white-space: nowrap;
      flex-shrink: 0;
    }
  }

  /* 宽度不够时：标题一行、按钮整组换到下一行 */
  @container (max-width: 780px) {
    .header-left {
      flex: 1 1 100%;
      width: 100%;
    }

    .header-actions {
      flex: 1 1 100%;
      width: 100%;
      margin-left: 0;
      justify-content: flex-start;
    }
  }
}

.btn-start {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px 2px 8px;
  //上 右 下 左
  border-radius: 4px;
  font-size: var(--font-size-md);
  outline: none !important;
  border: 1px solid var(--border-color);
  cursor: pointer;

  &:hover:not(:disabled) {
    // color: var(--color-primary);
    // border-color: var(--color-primary);
    background: var(--dropdown-hover-color);
  }

}

.btn-restart {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px 2px 8px;
  //上 右 下 左
  border-radius: 4px;
  font-size: var(--font-size-md);
  outline: none !important;
  border: 1px solid var(--border-color);
  cursor: pointer;

  &:hover:not(:disabled) {
    // color: var(--btn-warning-hover-border);
    // border-color: var(--btn-warning-hover-border);
    background: var(--dropdown-hover-color);
  }
}

.btn-delete,
.btn-stop {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px 2px 8px;
  //上 右 下 左
  border-radius: 4px;
  font-size: var(--font-size-md);
  outline: none !important;
  border: 1px solid var(--border-color);
  cursor: pointer;

  &:hover:not(:disabled) {
    background: var(--dropdown-hover-color);
  }
}

.btn-edit,
.btn-update {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px 2px 8px;
  //上 右 下 左
  border-radius: 4px;
  font-size: var(--font-size-md);
  outline: none !important;
  border: 1px solid var(--border-color);
  cursor: pointer;

  &:hover:not(:disabled) {
    background: var(--dropdown-hover-color);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border-radius: 4px;
  font-size: var(--font-size-md);
  outline: none !important;

  &.btn-loading {
    opacity: 0.8;
    pointer-events: none;
  }

  &:focus,
  &:focus-visible,
  &:active {
    outline: none !important;
    box-shadow: none !important;
    border-color: transparent !important;
  }

  &-outline-danger {
    background: transparent;
    border-color: var(--btn-danger-border);
    color: var(--btn-danger-border);

    &:hover:not(:disabled) {
      background: var(--btn-danger-hover-bg);
      color: var(--btn-danger-hover-text);
      border-color: var(--btn-danger-hover-border);
    }
  }

  &-sm {
    padding: 5px 12px;
    font-size: var(--font-size-sm);
  }

  &-warning {
    background: var(--btn-warning-bg);
    color: var(--btn-primary-text);
    border-color: var(--btn-warning-border);

    &:hover:not(:disabled) {
      background: var(--btn-warning-hover-bg);
      color: var(--btn-primary-text);
      border-color: var(--btn-warning-hover-border);
    }

    &:disabled {
      background: var(--btn-warning-disabled-bg);
      color: var(--btn-warning-disabled-text);
      border-color: var(--btn-warning-disabled-border);
    }
  }
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

.spin-icon {
  animation: spin 0.8s linear infinite;
}

.service-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 10px;
  margin-top: 2px;
}

.service-card {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 16px 18px 14px;
  border-radius: 16px;
  // background: linear-gradient(125deg, #f4f7ff 0%, #fafbff 40%, #ffffff 74%);
  border: 1px solid rgba(99, 102, 241, 0.10);
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.04);
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
  cursor: default;
  overflow: hidden;

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 10px 24px rgba(16, 24, 40, 0.10);
    border-color: rgba(99, 102, 241, 0.08);
  }

  :deep(.el-tag) {
    font-size: var(--font-size-sm);
  }

  &.is-disabled {
    opacity: 0.6;
    filter: grayscale(0.25);
  }

  &.manage-mode {
    cursor: pointer;

    &.is-selected {
      border-color: var(--color-primary);
      box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.18);
    }
  }

}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 4px;

  .status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .card-info {
    flex: 1;

    h3 {
      font-size: var(--font-size-base);
      font-weight: 600;
      margin: 0;
    }
  }

  .card-meta {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }
}

:deep(.el-select__wrapper) {
  min-height: 32px !important;
}

.card-desc {
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  margin-bottom: 10px;
  //height: 36px;
  //line-height: 36px; /* 行高等于高度实现垂直居中+单行 */
  white-space: nowrap;
  /* 强制单行不换行 */
  overflow: hidden;
  /* 超出部分隐藏 */
  text-overflow: ellipsis;
  /* 超出显示省略号 */

}


.card-error {
  font-size: var(--font-size-sm);
  color: var(--color-danger);
  background: rgba(255, 77, 79, 0.08);
  padding: 0px 0px;
  border-radius: 6px;
  margin-bottom: 0px;
  // height: 30px;
  white-space: nowrap;
  /* 强制单行不换行 */
  overflow: hidden;
  /* 超出部分隐藏 */
  text-overflow: ellipsis;
  /* 超出显示省略号 */
}

.card-actions {
  display: flex;
  gap: 8px;
}

.empty-state {
  text-align: center;
  padding: 80px 20px;

  .empty-icon {
    display: flex;
    justify-content: center;
    margin-bottom: 16px;

    svg {
      width: 70px;
      height: 70px;
    }
  }

  .empty-title {
    font-size: 18px;
    font-weight: 500;
    margin-bottom: 8px;
  }

  .empty-desc {
    font-size: var(--font-size-base);
    color: var(--text-secondary);
    margin-bottom: 24px;
    margin-left: auto;
    margin-right: auto;
  }

  .empty-actions {
    display: flex;
    gap: 12px;
    justify-content: center;
  }
}

.overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.15);
  backdrop-filter: blur(0px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  z-index: 200;
}

.dialog {
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.06), transparent), var(--theme-dailog-color);
  color: var(--text-primary);
  padding: 28px;
  border-radius: 18px;
  width: 480px;
  border: 1px solid var(--border-color);
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);

  h3 {
    font-size: var(--font-size-lg);
    font-weight: 700;
    margin-bottom: 20px;
  }
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;

  h3 {
    font-size: 17px;
    margin: 0;
  }
}

.dialog-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;

  transition: all 0.2s;
  flex-shrink: 0;

  &:hover {
    background: var(--dropdown-hover-color);
  }
}

.form-group {
  margin-bottom: 14px;

  label {
    display: block;
    font-size: var(--font-size-md);
    font-weight: 500;
    margin-bottom: 6px;
    color: var(--text-secondary);
  }

  input,
  textarea,
  select {
    width: 100%;
    padding: 9px 12px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    font-size: var(--font-size-base);
    outline: none;
    font-family: inherit;
    background: transparent;
  }

  .number-input-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;

    input[type="number"] {
      width: 100%;
      padding-right: 28px;
      -moz-appearance: textfield;

      &::-webkit-inner-spin-button,
      &::-webkit-outer-spin-button {
        -webkit-appearance: none;
        margin: 0;
      }
    }

    .number-arrows {
      position: absolute;
      right: 1px;
      top: 1px;
      bottom: 1px;
      width: 26px;
      display: flex;
      flex-direction: column;
      opacity: 0;
      transition: opacity 0.2s;
      border-radius: 0 5px 5px 0;
      overflow: hidden;
    }

    &:hover .number-arrows {
      opacity: 1;
    }

    .number-arrow {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      background: transparent;
      border: none;
      color: var(--text-primary);
      cursor: pointer;
      padding: 0;

      &:hover {
        background: rgba(128, 128, 128, 0.2);
      }

      svg {
        width: 12px;
        height: 12px;
      }
    }
  }
}

.form-row {
  display: flex;
  gap: 12px;

  .form-group {
    flex: 1;
  }
}

.form-check {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 20px;

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--color-primary);
  }

  label {
    font-size: var(--font-size-md);
    cursor: pointer;
  }
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;

  .btn,
  .btn-cancel,
  .btn-primary {
    min-width: 76px;
    height: 36px;
    padding: 0 16px;
    border-radius: 4px;
    font-weight: 600;
    box-sizing: border-box;
  }

  .btn-cancel {
    color: var(--btn-secondary-text);
    background: var(--cancel-btn-color);
    border: 1px solid var(--btn-secondary-border);

    &:hover:not(:disabled) {
      background: var(--cancel-btn-hover-color);
      color: var(--btn-secondary-hover-text);
      border-color: var(--btn-secondary-hover-border);
    }
  }
}

.upload-zone {
  border: 2px dashed var(--border-color);
  border-radius: 12px;
  padding: 32px;
  text-align: center;
  cursor: pointer;
  transition: all 0.2s;

  &:hover,
  &.dragging {
    border-color: var(--color-primary);
    background: rgba(24, 144, 255, 0.02);
  }

  &.has-file {
    border-style: solid;
    border-color: var(--color-success);
    background: rgba(82, 196, 26, 0.04);
  }

  .upload-hint {
    font-size: var(--font-size-base);
    color: var(--text-secondary);
    margin-top: 12px;
  }

  .upload-formats {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    opacity: 0.6;
    margin-top: 4px;
  }

  .file-info {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: center;
    flex-direction: column;

    .file-icon {
      font-size: 28px;
    }

    .file-name {
      font-size: var(--font-size-base);
      font-weight: 500;
    }

    .file-path {
      font-size: 11px;
      color: var(--text-secondary);
      word-break: break-all;
      max-width: 100%;
    }

    .file-size {
      font-size: var(--font-size-sm);
      color: var(--text-secondary);
    }
  }
}

// 依赖服务 Tab
.dep-services-wrap {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding-top: 10px;
}

.dep-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  flex-shrink: 0;
  flex-wrap: wrap;
  gap: 12px;
}

// 控件的选中态默认使用品牌色，在彩色背景下对比度不足。
// 只覆盖依赖服务区域的文字，保留复选框/开关本身的状态颜色。
.dep-services-wrap {
  :deep(.el-checkbox__label),
  :deep(.el-switch__label) {
    color: var(--text-primary) !important;
  }
}

.dep-header-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 12px;
  margin-left: auto;

  .el-button + .el-button {
    margin-left: 0;
  }
}

.dep-subtitle {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.dep-services-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 10px;
  overflow-y: auto;
  padding-right: 2px;
  align-content: start;
  // ★ 必须显式给 max-content。这是个「定高的滚动容器 + auto 行」的组合：
  //   卡片总高一旦超过可视高度，auto 行会被**压缩**到 min-content，而卡片的
  //   min-content 低于真实内容（.svc-meta/.svc-actions 都是 flex-wrap，能按
  //   单行算高；.svc-desc 更是 flex-shrink:1），于是卡片比内容矮 ——
  //   .svc-actions 是 flex-shrink:0 挤不进去，被 overflow:hidden 裁掉半截按钮。
  //   现象就是「窗口矮时按钮被切、全屏就正常」。max-content 的行不可压缩，
  //   超出交给 overflow-y 滚动。别删。
  grid-auto-rows: max-content;
}

.dep-service-card {
  display: flex;
  flex-direction: column;
  min-height: calc(var(--font-size-base) * 10);
  height: auto;
  box-sizing: border-box;
  padding: calc(var(--font-size-base) * 1.14);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  overflow: hidden;
  transition: border-color 0.2s, box-shadow 0.2s;

  &:hover {
    // transform: translateY(-2px);
    box-shadow: 0 10px 24px rgba(16, 24, 40, 0.10);
    border-color: rgba(99, 102, 241, 0.28);
  }

  :deep(.el-tag) {
    font-size: var(--font-size-sm);
  }

  &.not-installed {
    background: color-mix(in srgb, var(--bg-secondary) 88%, transparent);
    border-color: var(--border-color) !important;

    .svc-meta:not(.svc-recovery-settings),
    .svc-desc {
      opacity: 0.65;
    }
  }
}

.svc-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  flex-shrink: 0;
  min-width: 0;
}

.svc-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-secondary);
  flex-shrink: 0;

  &.running {
    background: var(--color-success);
  }
}

.svc-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.svc-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin-bottom: 6px;
  flex-shrink: 0;
  min-width: 0;
}

.svc-recovery-settings {
  :deep(.el-checkbox__inner) {
    background-color: transparent !important;
    border-color: var(--border-color) !important;
  }

  :deep(.el-checkbox__input.is-checked .el-checkbox__inner),
  :deep(.el-checkbox__input.is-indeterminate .el-checkbox__inner) {
    background-color: var(--color-primary) !important;
    border-color: var(--color-primary) !important;
  }

  :deep(.el-checkbox__input.is-checked .el-checkbox__inner::after) {
    border-color: #fff !important;
  }

  :deep(.el-checkbox__input.is-indeterminate .el-checkbox__inner::before) {
    background-color: #fff !important;
  }

  :deep(.el-checkbox__input.is-disabled .el-checkbox__inner) {
    background-color: transparent !important;
    border-color: var(--border-color) !important;
    opacity: 0.5;
  }

  :deep(.el-checkbox__input.is-disabled.is-checked .el-checkbox__inner) {
    background-color: var(--color-primary) !important;
    border-color: var(--color-primary) !important;
    opacity: 0.5;
  }
}

.no-pid {
  color: var(--text-tertiary);
}

.svc-desc {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin-bottom: 12px;
  line-height: 1.5;
  min-width: 0;
  overflow: hidden;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.svc-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: auto;
  flex-shrink: 0;
  min-width: 0;

  .btn-start,
  .btn-stop,
  .btn-edit,
  .btn-update {
    max-width: 100%;
  }
}

.fastkg-version-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  font-size: var(--font-size-md);
  color: var(--text-secondary);

  strong {
    color: var(--text-primary);
    font-weight: 600;
  }

  .is-latest {
    color: var(--color-primary);
  }
}

.fastkg-update-progress {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 8px 0;
}

// 编辑配置弹窗
.dep-edit-form {
  display: flex;
  flex-direction: column;
  gap: 12px;

  :deep(.el-input__wrapper),
  :deep(.el-input-number .el-input__wrapper) {
    background-color: transparent !important;
  }

  :deep(.el-input__inner) {
    background-color: transparent !important;
  }

  :deep(.el-input-number__decrease),
  :deep(.el-input-number__increase) {
    background-color: transparent !important;
  }
}

.dep-edit-item {
  display: flex;
  flex-direction: column;
  gap: 4px;

  label {
    font-size: var(--font-size-md);
    color: var(--text-secondary);
  }
}

.dep-edit-hint {
  font-size: var(--font-size-sm);
  color: var(--text-tertiary);
  margin: 0;
}

.dep-port-fallback,
.dep-port-attempt,
.dep-port-conflict {
  color: var(--color-warning);
}

.dep-port-attempt {
  font-weight: 500;
}

/* 添加/上传服务弹窗：去掉输入框内边框 */
.add-service-dialog :deep(.el-input__inner),
.upload-service-dialog :deep(.el-input__inner) {
  border: none !important;
  box-shadow: none !important;
}
</style>

<style>
/* WSP 列表原生表格：滚动条样式对齐全部工具（竖向仅表体） */
.wsp-view .wsp-table-body-scroll {
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;
}
.wsp-view .wsp-table-body-scroll::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
.wsp-view .wsp-table-body-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.wsp-view .wsp-table-body-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15));
  border-radius: 3px;
}

/* 情景主题（除雾灰调）：白字 + 卡片白色10% */
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]),
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .header-stats,
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .card-meta,
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .card-desc,
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]).wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .metric-label {
  color: #fff !important;
}

.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .service-card {
  background: rgba(255, 255, 255, 0.10) !important;
  border-color: rgba(255, 255, 255, 0.15) !important;
}

.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dep-service-card {
  background: rgba(255, 255, 255, 0.10) !important;
}

.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .btn-danger {
  color: #fff !important;
}

/* 浅色/雾灰调：黑字 + 卡片黑色5% */
.wsp-view[data-theme-id="simple-light"],
.wsp-view[data-theme-id="simple-light"] .header-stats,
.wsp-view[data-theme-id="simple-light"] .card-meta,
.wsp-view[data-theme-id="simple-light"] .card-desc,
.wsp-view[data-theme-id="simple-light"].wsp-view[data-theme-id="simple-light"] .metric-label,
.wsp-view[data-theme-id="scene-graphite-cyan"],
.wsp-view[data-theme-id="scene-graphite-cyan"] .header-stats,
.wsp-view[data-theme-id="scene-graphite-cyan"] .card-meta,
.wsp-view[data-theme-id="scene-graphite-cyan"] .card-desc,
.wsp-view[data-theme-id="scene-graphite-cyan"].wsp-view[data-theme-id="scene-graphite-cyan"] .metric-label {
  color: #1f2937 !important;
}

/* .wsp-view[data-theme-id="simple-light"] .service-card, */
.wsp-view[data-theme-id="scene-graphite-cyan"] .service-card {
  background: rgba(0, 0, 0, 0.05) !important;
  border-color: rgba(0, 0, 0, 0.07) !important;
}

.wsp-view[data-theme-id="scene-graphite-cyan"] .dep-service-card {
  background: rgba(0, 0, 0, 0.05) !important;
}

/* 暗色：白字 + 卡片白色5% */
.dark .wsp-view,
.dark .wsp-view .header-stats,
.dark .wsp-view .card-meta,
.dark .wsp-view .card-desc,
.dark .wsp-view.dark .wsp-view .metric-label {
  color: #fff !important;
}

.dark .wsp-view .service-card {
  background: rgba(255, 255, 255, 0.05) !important;
  border-color: rgba(255, 255, 255, 0.07) !important;
}

.dark .wsp-view .dep-service-card {
  background: rgba(255, 255, 255, 0.05) !important;
}

.dark .wsp-view .dialog {
  color: #ffffff;
}

.dark .wsp-view .dialog h3 {
  color: #ffffff;
}

.dark .wsp-view .dialog label {
  color: #cbd5e0;
}

.dark .wsp-view .dialog input,
.dark .wsp-view .dialog textarea,
.dark .wsp-view .dialog select {
  background: transparent;
  color: #fff;
  border-color: rgba(255, 255, 255, 0.2);
}

.dark .wsp-view .dialog select option {
  background: #1a1a1a;
  color: #fff;
}

.dark .wsp-view .dialog-close {
  color: #cbd5e0;
}

.dark .wsp-view .dialog-close:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #ffffff;
}

/* 分类 el-select 触发器和暗色适配 */
.wsp-view .category-select .el-select__wrapper {
  height: 43px !important;
  border: none !important;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.08);
  font-size: var(--font-size-base);
  box-shadow: none !important;
}

.wsp-view .category-select .el-select__wrapper.is-focus {
  border-color: transparent !important;
}

.dark .wsp-view .category-select .el-select__wrapper {
  border-color: transparent !important;
}

/* 分类 el-cascader 触发器：去除输入框边框、背景透明 */
.wsp-view .category-cascader .el-input__wrapper {
  border: none !important;
  box-shadow: none !important;
  background: transparent !important;
}

.wsp-view .category-cascader .el-input__inner {
  border: none !important;
  box-shadow: none !important;
  background: transparent !important;
}

/* 情景主题（除雾灰调）：分类下拉框选中文本白色 */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select .el-select__placeholder,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select .el-select__selected-item {
  color: #fff !important;
}

/* 情景主题（除雾灰调）：上传弹窗分类下拉白底黑字 */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dialog .form-group label,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dialog .upload-hint,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dialog .upload-formats,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dialog .file-name,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dialog .file-path,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .dialog .form-group input {
  color: #fff !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select-popper,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select-popper.el-popper,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select-popper .el-select-dropdown {
  --el-bg-color-overlay: var(--theme-dailog-color) !important;
  --el-popper-bg-color-light: var(--theme-dailog-color) !important;
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
  border-color: var(--border-color) !important;
}

/* html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select-popper .el-select-dropdown__item {
  color: #1f2937 !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select-popper .el-select-dropdown__item:hover {
  background: #f3f4f6 !important;
}

html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .category-select-popper .el-select-group__title {
  color: #6b7280 !important;
} */

/* 情景主题（除雾灰调）下 toolbar 边框统一 */
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .toolbar-wrap {
  border-color: rgba(255, 255, 255, 0.1) !important;
}

/* 情景主题（除雾灰调）下搜索框图标和提示文字白色 */
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .toolbar-wrap .el-input__inner::placeholder {
  color: rgba(255, 255, 255, 0.6) !important;
}

.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .toolbar-wrap .el-input__prefix,
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .toolbar-wrap .el-input__suffix {
  color: rgba(255, 255, 255, 0.7) !important;
}

/* toolbar 搜索框和筛选下拉框样式 */
.wsp-view .toolbar-wrap .el-input__wrapper,
.wsp-view .toolbar-wrap .el-select__wrapper {
  background: transparent !important;
  font-size: var(--font-size-sm) !important;
  border-color: var(--border-color) !important;
}

/* 表格错误行背景 */
.wsp-view .wsp-native-table tr.row-error td {
  background: rgba(255, 77, 79, 0.05) !important;
}

/* 情景主题（除雾灰调）下表格样式 */
.wsp-view[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .el-table {
  --el-table-bg-color: transparent !important;
  --el-table-tr-bg-color: transparent !important;
  --el-table-header-bg-color: transparent !important;
  --el-table-text-color: #fff !important;
  --el-table-header-text-color: rgba(255, 255, 255, 0.85) !important;
  --el-table-border-color: rgba(255, 255, 255, 0.1) !important;
  --el-table-row-hover-bg-color: rgba(255, 255, 255, 0.05) !important;
}
</style>

<style lang="scss">
/* ── 首次使用引导（全局，因使用 Teleport 挂载到 body） ── */
.wsp-guide-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  pointer-events: none;
}

.wsp-guide-mask {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  pointer-events: auto;
}

.wsp-guide-highlight {
  position: fixed;
  border-radius: 8px;
  border: 2px solid var(--color-primary);
  // box-shadow: 0 0 0 4px rgba(22, 119, 255, 0.2), 0 0 20px rgba(22, 119, 255, 0.3);
  pointer-events: none;
  z-index: 1;
}

.wsp-guide-tooltip {
  position: fixed;
  background: var(--theme-dailog-color);
  color: var(--text-primary, #1f2937);
  border-radius: 10px;
  padding: 16px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.2);
  z-index: 2;
  min-width: 280px;
  max-width: 320px;
  pointer-events: auto;
}

.wsp-guide-arrow {
  position: absolute;
  width: 12px;
  height: 12px;
  background: inherit;
  transform: rotate(45deg);
  /* 只露出弹窗外的半个菱形，避免半透明背景叠在弹窗上变深 */
  clip-path: polygon(0 0, 100% 0, 0 100%);
}

.wsp-guide-step {
  font-size: 12px;
  color: var(--text-secondary, #6b7280);
  margin-bottom: 6px;
}

.wsp-guide-title {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--text-primary, #1f2937);
}

.wsp-guide-content {
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-secondary, #4b5563);
  margin-bottom: 16px;
}

.wsp-guide-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.wsp-guide-btn {
  padding: 7px 16px;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  border: 1px solid var(--border-color, #d1d5db);
  background: transparent;
  color: var(--text-primary, #1f2937);
  transition: all 0.15s ease;

  &:hover {
    background: var(--bg-hover, rgba(0, 0, 0, 0.05));
  }
}

.wsp-guide-btn-next,
.wsp-guide-btn-finish {
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}
</style>

<style lang="scss">
/* 遮罩层：禁止出现窗口最右侧垂直滚动条 */
.dep-edit-dialog-overlay.el-overlay,
.dep-edit-dialog-overlay {
  overflow: hidden !important;
}

.dep-edit-dialog-overlay .el-overlay-dialog {
  overflow: hidden !important;
}

/* 弹窗打开时，隐藏背后依赖服务列表滚动条，避免透出遮罩右侧 */
body.el-popup-parent--hidden .dep-services-grid {
  overflow: hidden !important;
}

/* 编辑服务配置 / 恢复记录弹窗关闭按钮：与上传 WSP 服务包 .dialog-close 一致 */
.dep-edit-dialog.el-dialog .el-dialog__headerbtn {
  width: 32px !important;
  height: 32px !important;
  top: 16px !important;
  right: 16px !important;
  display: inline-flex !important;
  align-items: center !important;
  justify-content: center !important;
  border: none !important;
  background: transparent !important;
  border-radius: 8px !important;
  padding: 0 !important;
  box-shadow: none !important;
  outline: none !important;
  transition: all 0.2s;
}

.dep-edit-dialog.el-dialog .el-dialog__headerbtn:hover {
  background: var(--dropdown-hover-color, var(--bg-hover)) !important;
}

.dep-edit-dialog.el-dialog .el-dialog__headerbtn .el-dialog__close {
  color: var(--text-secondary) !important;
  font-size: 18px !important;
}

.dep-edit-dialog.el-dialog .el-dialog__headerbtn:hover .el-dialog__close {
  color: var(--text-primary) !important;
}

/* 分类级联下拉箭头：与下拉窗口同色同边框；修复 EP is-light 的 right:0 错位导致整颗菱形露边 */
.category-cascader-popper.el-popper,
.category-cascader-popper.el-cascader__dropdown {
  --el-bg-color: var(--theme-dailog-color) !important;
  --el-bg-color-overlay: var(--theme-dailog-color) !important;
  --el-popper-bg-color-light: var(--theme-dailog-color) !important;
  --el-border-color-light: var(--border-color) !important;
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
  border: 1px solid var(--border-color) !important;
  overflow: visible !important;
}

.category-cascader-popper .el-cascader-panel {
  position: relative;
  z-index: 1;
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
}

/* 箭头在面板后方，只露出顶部小三角 */
.category-cascader-popper.el-popper > .el-popper__arrow {
  z-index: -1 !important;
}

.category-cascader-popper[data-popper-placement^='bottom'] > .el-popper__arrow {
  top: -5px !important;
}

.category-cascader-popper[data-popper-placement^='top'] > .el-popper__arrow {
  bottom: -5px !important;
}

.category-cascader-popper.el-popper .el-popper__arrow::before,
.category-cascader-popper.el-cascader__dropdown .el-popper__arrow::before,
.category-cascader-popper.el-popper.is-light > .el-popper__arrow::before {
  background: var(--theme-dailog-color) !important;
  background-color: var(--theme-dailog-color) !important;
  /* 与下拉窗口边框完全一致 */
  border: 1px solid var(--border-color) !important;
  box-shadow: none !important;
  /* 覆盖 EP .is-light > .el-popper__arrow::before { right: 0 } 造成的菱形错位 */
  right: auto !important;
  left: 0 !important;
}

/* 底部弹出：内侧两边透明，只留朝外的小三角描边 */
.category-cascader-popper[data-popper-placement^='bottom'] > .el-popper__arrow::before {
  border-bottom-color: transparent !important;
  border-right-color: transparent !important;
}

.category-cascader-popper[data-popper-placement^='top'] > .el-popper__arrow::before {
  border-top-color: transparent !important;
  border-left-color: transparent !important;
}

/* 浅色主题 / 雾灰调：状态筛选下拉里的标签文字强制白色（压过全局 .el-select-dropdown__item span 的深色字） */
html.light[data-theme-id="simple-light"] .wsp-status-filter-popper .el-tag.wsp-status-option-tag,
html.light[data-theme-id="simple-light"] .wsp-status-filter-popper .el-tag.wsp-status-option-tag span,
html.light[data-theme-id="simple-light"] .wsp-status-filter-popper .el-select-dropdown__item .el-tag,
html.light[data-theme-id="simple-light"] .wsp-status-filter-popper .el-select-dropdown__item .el-tag span,
html.light[data-theme-id="scene-graphite-cyan"] .wsp-status-filter-popper .el-tag.wsp-status-option-tag,
html.light[data-theme-id="scene-graphite-cyan"] .wsp-status-filter-popper .el-tag.wsp-status-option-tag span,
html.light[data-theme-id="scene-graphite-cyan"] .wsp-status-filter-popper .el-select-dropdown__item .el-tag,
html.light[data-theme-id="scene-graphite-cyan"] .wsp-status-filter-popper .el-select-dropdown__item .el-tag span {
  color: #fff !important;
}

/* 情景主题（除雾灰调）：压过 ChatView/useLog 对 .el-tag 的半透明白覆盖，恢复状态彩色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-tag.wsp-status-option-tag,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-select-dropdown__item .el-tag,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-tag.wsp-status-option-tag span,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-select-dropdown__item .el-tag span {
  color: #fff !important;
  border: none !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-tag--success {
  background-color: var(--el-color-success) !important;
  border-color: var(--el-color-success) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-tag--warning {
  background-color: var(--el-color-warning) !important;
  border-color: var(--el-color-warning) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-tag--info {
  background-color: var(--el-color-info) !important;
  border-color: var(--el-color-info) !important;
}
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .wsp-status-filter-popper .el-tag--danger {
  background-color: var(--el-color-danger) !important;
  border-color: var(--el-color-danger) !important;
}
</style>

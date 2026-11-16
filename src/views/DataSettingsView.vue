<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { ElMessageBox, ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const appDataPath = ref('')
const appLogPath = ref('')

const loadPaths = async () => {
  try {
    appDataPath.value = await invoke('get_exe_dir')
    appLogPath.value = await invoke('get_db_dir')
  } catch (e) {
    console.error('Failed to load paths:', e)
    appDataPath.value = 'data'
    appLogPath.value = 'data'
  }
}

const savePath = async (key: string, value: string, description: string) => {
  try {
    await invoke('update_setting_with_description', {
      key,
      value,
      description,
      category: 'datasettings'
    })
  } catch (e) {
    console.error('Failed to save path:', e)
  }
}

const openDir = async (path: string) => {
  try {
    await invoke('open_directory', { path })
  } catch (e) {
    console.error('Failed to open directory:', e)
  }
}

const changePath = async (type: 'appData' | 'appLog') => {
  const currentPath = type === 'appData' ? appDataPath.value : appLogPath.value
  const desc = type === 'appData' ? '程序目录数据' : '数据库数据'
  try {
    const { value: input } = await ElMessageBox.prompt('请输入完整路径', '修改路径', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      inputValue: currentPath,
      appendTo: document.body,
    })
    if (input && input !== currentPath) {
      const cleanPath = input.replace(/^"|"$/g, '').trim()
      if (type === 'appData') {
        appDataPath.value = cleanPath
        await savePath('app_data_path', cleanPath, desc)
      } else {
        appLogPath.value = cleanPath
        await savePath('app_log_path', cleanPath, desc)
      }
    }
  } catch {
    // 用户点击取消，不做任何操作
  }
}

const backupVisible = ref(false)
const restoreVisible = ref(false)
const resetVisible = ref(false)
const clearCacheVisible = ref(false)

const handleBackup = async (type: 'appData' | 'dbData') => {
  backupVisible.value = false
  const sourcePath = type === 'appData' ? appDataPath.value : appLogPath.value
  const backupTypeName = type === 'appData' ? '程序目录数据' : '数据库数据'

  try {
    const backupPath: string = await invoke('backup_directory', {
      sourcePath,
      backupType: backupTypeName
    })
    // 使用 ElMessageBox 显示成功提示，添加"打开备份路径"按钮
    await ElMessageBox.alert(
      `${t('dataSettings.backupFile')}${backupPath}`,
      t('dataSettings.backupSuccess'),
      {
        confirmButtonText: t('dataSettings.openBackupPath'),
        cancelButtonText: t('dataSettings.close'),
        showCancelButton: true,
        type: 'success',
        appendTo: document.body,
        customClass: 'backup-success-dialog'
      }
    ).then(() => {
      // 点击"打开备份路径"按钮
      openDir(sourcePath)
    }).catch(() => {
      // 点击"关闭"或取消
    })
  } catch (e: any) {
    console.error('Backup failed:', e)
    // 根据错误信息判断失败原因
    const errorMsg = e.message || String(e)
    let displayMsg = errorMsg
    if (errorMsg.includes('无数据')) {
      displayMsg = t('dataSettings.backupFailedNoData')
    }
    // 使用 ElMessageBox 显示失败提示
    await ElMessageBox.alert(
      displayMsg,
      t('dataSettings.backupFailed'),
      { confirmButtonText: t('dataSettings.confirm'), type: 'error', appendTo: document.body, customClass: 'backup-error-dialog' }
    )
  }
}

const showRestoreConfirm = ref(false)
const restoreConfirmInfo = ref<any>(null)
const showResetConfirm = ref(false)
const resetConfirmType = ref('')
const resetConfirmLabel = ref('')


// 备份文件信息类型
interface BackupFileInfo {
  file_name: string
  file_size: number
  file_size_formatted: string
  created_time: string
  backup_type: string
}

const handleRestore = async (type: 'appData' | 'dbData') => {
  restoreVisible.value = false
  const targetPath = type === 'appData' ? appDataPath.value : appLogPath.value
  const restoreTypeName = type === 'appData' ? t('dataSettings.restoreAppData') : t('dataSettings.restoreDbData')
  let loadingMsg: any = null

  // 选择备份文件（从对应目录打开文件选择器）
  try {
    const selected: string | null = await invoke('select_backup_file', {
      defaultDir: targetPath
    })
    if (!selected) return

    // 获取备份文件信息
    let backupInfo: BackupFileInfo
    try {
      backupInfo = await invoke('get_backup_file_info', { backupPath: selected })
    } catch (e) {
      console.warn('获取备份文件信息失败:', e)
      backupInfo = {
        file_name: selected.split('\\').pop() || selected,
        file_size: 0,
        file_size_formatted: t('dataSettings.unknown'),
        created_time: t('dataSettings.unknown'),
        backup_type: restoreTypeName
      }
    }

    // 使用 restoreTypeName 作为类型显示（已经是国际化后的文本）
    const displayType = restoreTypeName
    const confirmMsg = `${t('dataSettings.backupFileInfo')}：\n${t('dataSettings.fileName')}：${backupInfo.file_name}\n${t('dataSettings.fileSize')}：${backupInfo.file_size_formatted}\n${t('dataSettings.createdTime')}：${backupInfo.created_time}\n${t('dataSettings.type')}：${displayType}\n\n${t('dataSettings.restoreWarning', { type: restoreTypeName })}`

    restoreConfirmInfo.value = { msg: confirmMsg, type: restoreTypeName, selected, targetPath }
    showRestoreConfirm.value = true
  } catch (e: any) {
    loadingMsg?.close()
    if (e === 'cancel') return
    console.error('Restore failed:', e)
    await ElMessageBox.alert(
      e.message || String(e),
      t('dataSettings.restoreFailed'),
      { confirmButtonText: t('dataSettings.confirm'), type: 'error', appendTo: document.body }
    )
  }
}

const doRestore = async (skipBackup: boolean) => {
  showRestoreConfirm.value = false
  const info = restoreConfirmInfo.value
  if (!info) return

  let loadingMsg: any = null
  try {
    loadingMsg = ElMessage({
      message: `正在恢复${info.type}，请稍候...`,
      type: 'info',
      duration: 0,
      zIndex: 999999
    })

    const result: string = await invoke('restore_from_backup', {
      backupPath: info.selected,
      targetPath: info.targetPath,
      restoreType: info.type,
      skipPreBackup: skipBackup
    })

    loadingMsg.close()

    const parts = (result as string).split('|')
    const preBackupPath = parts.length > 1 ? parts[1] : ''
    const successCode = parts[0]
    // 处理成功消息码
    const successMsg = successCode === 'RESTORE_SUCCESS' ? t('dataSettings.restoreSuccessMsg') : successCode

    if (preBackupPath) {
      await ElMessageBox.confirm(
        `${successMsg}\n${t('dataSettings.preRestoreBackup')}: ${preBackupPath}`,
        t('dataSettings.restoreSuccess'),
        { confirmButtonText: t('dataSettings.deleteBackup'), cancelButtonText: t('dataSettings.keep'), type: 'success', appendTo: document.body }
      ).then(async () => { await invoke('delete_file', { path: preBackupPath }) }).catch(() => { })
    } else {
      await ElMessageBox.alert(successMsg, t('dataSettings.restoreSuccess'),
        { confirmButtonText: t('dataSettings.confirm'), type: 'success', appendTo: document.body })
    }
    await loadPaths()
  } catch (e: any) {
    loadingMsg?.close()
    if (e === 'cancel') return
    // 处理错误码
    let errorMsg = e.message || String(e)
    // 移除后端的 "IO 错误:" 或 "业务错误:" 前缀（使用全局替换确保所有出现都被移除）
    errorMsg = errorMsg.replace(/IO 错误:\s*/g, '').replace(/业务错误:\s*/g, '')
    if (errorMsg.includes('INVALID_BACKUP_FILE')) {
      errorMsg = t('dataSettings.invalidBackupFile')
    } else if (errorMsg.includes('RESTORE_ERROR')) {
      // 解析 RESTORE_ERROR 错误码
      const parts = errorMsg.split('|')
      if (parts.length >= 2) {
        const originalError = parts[1]
        const preBackupPath = parts.length >= 3 ? parts[2] : ''
        errorMsg = t('dataSettings.ioError') + originalError
        if (preBackupPath) {
          errorMsg += '\n' + t('dataSettings.preRestoreBackupKept') + ': ' + preBackupPath
        }
      }
    }
    await ElMessageBox.alert(errorMsg, t('dataSettings.restoreFailed'),
      { confirmButtonText: t('dataSettings.confirm'), type: 'error', appendTo: document.body })
  }
}

const resetLabels: Record<string, string> = {
  appData: t('dataSettings.resetAppData'),
  dbData: t('dataSettings.resetDbData'),
  all: t('dataSettings.resetAllData')
}

const openResetConfirm = (type: string) => {
  resetVisible.value = false
  resetConfirmType.value = type
  resetConfirmLabel.value = resetLabels[type] || type
  showResetConfirm.value = true
}

const RESET_TIMEOUT = 60000 // 60秒超时

const doReset = async (skipBackup: boolean) => {
  showResetConfirm.value = false
  const label = resetConfirmLabel.value
  const msg = ElMessage({ message: t('dataSettings.resetting'), type: 'info', duration: 0, zIndex: 999999 })
  try {
    // 备份到源目录，记录文件名传给 reset 跳过删除
    let keepFiles: string[] = []
    if (!skipBackup) {
      const resetType = resetConfirmType.value
      if (resetType === 'appData' || resetType === 'all') {
        const p: string = await invoke('backup_directory', { sourcePath: appDataPath.value, backupType: t('dataSettings.resetAppData') + '(重置前备份)' })
        keepFiles.push(p.split('\\').pop() || '')
      }
      if (resetType === 'dbData' || resetType === 'all') {
        const p: string = await invoke('backup_directory', { sourcePath: appLogPath.value, backupType: t('dataSettings.resetDbData') + '(重置前备份)' })
        keepFiles.push(p.split('\\').pop() || '')
      }
    }
    // 带超时的重置调用，防止后端卡死
    await Promise.race([
      invoke('reset_data', { resetType: resetConfirmType.value, keepFiles }),
      new Promise((_, reject) => setTimeout(() => reject(new Error('重置超时，请稍后重试')), RESET_TIMEOUT))
    ])
    msg.close()
    // 重新加载菜单
    const { useMenuStore } = await import('@/stores/menu')
    useMenuStore().loadMenus()
    await loadPaths()
    await ElMessageBox.alert(t('dataSettings.resetSuccess'), t('dataSettings.resetSuccess'),
      { confirmButtonText: t('dataSettings.confirm'), type: 'success', appendTo: document.body })
  } catch (e: any) {
    msg.close()
    await ElMessageBox.alert(e.message || String(e), t('dataSettings.resetFailed'),
      { confirmButtonText: t('dataSettings.confirm'), type: 'error', appendTo: document.body })
  }
}

const cacheItems = [
  { value: 'browser', label: '浏览器缓存', desc: '清除 WebView 浏览器 HTTP 缓存' },
  { value: 'db', label: '数据库缓存', desc: '收缩数据库日志文件释放空间' },
  { value: 'app', label: '程序临时文件', desc: '清理程序目录中的临时文件' },
  { value: 'kg', label: '知识库临时文件', desc: '清理知识库索引临时文件' },
  { value: 'all', label: '全部缓存', desc: '清除以上所有缓存数据' },
]

const doClearCache = async (type: string) => {
  clearCacheVisible.value = false
  const item = cacheItems.find(i => i.value === type)
  try {
    await ElMessageBox.confirm(
      `即将清除"${item?.label}"，是否继续？`,
      '清除缓存确认',
      { confirmButtonText: '确定清除', cancelButtonText: '取消', type: 'warning', appendTo: document.body }
    )
  } catch { return }
  const msg = ElMessage({ message: '正在清理缓存...', type: 'info', duration: 0, zIndex: 999999 })
  try {
    const result: string = await invoke('clear_cache', { cacheType: type })
    // 浏览器缓存需额外清理 localStorage
    if (type === 'browser' || type === 'all') {
      localStorage.clear()
      // 保留语言设置
      const lang = localStorage.getItem('fastclaw-language')
      if (lang) localStorage.setItem('fastclaw-language', lang)
    }
    msg.close()
    await ElMessageBox.alert(result || '缓存清理完成', '清理成功',
      { confirmButtonText: '确定', type: 'success', appendTo: document.body }).catch(() => { })
  } catch (e: any) {
    msg.close()
    if (e === 'cancel' || !e) return
    await ElMessageBox.alert(e.message || String(e), '清理失败',
      { confirmButtonText: '确定', type: 'error', appendTo: document.body }).catch(() => { })
  }
}

onMounted(() => {
  loadPaths()
})
</script>

<template>
  <div class="data-view-container">
    <div class="data-content">
      <div class="data-card">
        <div class="card-header">
          <span class="card-title">{{ $t('dataSettings.title') }}</span>
        </div>
        <div class="card-body">
          <div class="setting-row">
            <span class="row-label">{{ $t('dataSettings.backupRestore') }}</span>
            <div class="row-actions">
              <el-popover v-model:visible="backupVisible" trigger="click" placement="bottom-start" :width="260"
                popper-class="data-action-popover">
                <template #reference>
                  <button class="btn-action">
                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                      <polyline points="17 8 12 3 7 8"></polyline>
                      <line x1="12" y1="3" x2="12" y2="15"></line>
                    </svg>
                    {{ $t('dataSettings.backup') }}
                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"
                      style="margin-left: 4px;">
                      <polyline points="6 9 12 15 18 9"></polyline>
                    </svg>
                  </button>
                </template>
                <div class="backup-menu-item" @click="handleBackup('appData')">
                  <span class="menu-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                      stroke-linecap="round" stroke-linejoin="round">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                    </svg>
                  </span>
                  <div class="menu-content">
                    <div class="menu-title">{{ $t('dataSettings.backupAppData') }}</div>
                    <div class="menu-desc">{{ $t('dataSettings.backupAppDataDesc') }}</div>
                  </div>
                </div>
                <div class="backup-menu-divider"></div>
                <div class="backup-menu-item" @click="handleBackup('dbData')">
                  <span class="menu-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                      stroke-linecap="round" stroke-linejoin="round">
                      <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                      <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
                      <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
                    </svg>
                  </span>
                  <div class="menu-content">
                    <div class="menu-title">{{ $t('dataSettings.backupDbData') }}</div>
                    <div class="menu-desc">{{ $t('dataSettings.backupDbDataDesc') }}</div>
                  </div>
                </div>
              </el-popover>
              <el-popover v-model:visible="restoreVisible" trigger="click" placement="bottom-end" :width="260"
                popper-class="data-action-popover">
                <template #reference>
                  <button class="btn-action">
                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                      <polyline points="7 10 12 15 17 10"></polyline>
                      <line x1="12" y1="15" x2="12" y2="3"></line>
                    </svg>
                    {{ $t('dataSettings.restore') }}
                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"
                      style="margin-left: 4px;">
                      <polyline points="6 9 12 15 18 9"></polyline>
                    </svg>
                  </button>
                </template>
                <div class="backup-menu-item" @click="handleRestore('appData')">
                  <span class="menu-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                      stroke-linecap="round" stroke-linejoin="round">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                    </svg>
                  </span>
                  <div class="menu-content">
                    <div class="menu-title">{{ $t('dataSettings.restoreAppData') }}</div>
                    <div class="menu-desc">{{ $t('dataSettings.restoreAppDataDesc') }}</div>
                  </div>
                </div>
                <div class="backup-menu-divider"></div>
                <div class="backup-menu-item" @click="handleRestore('dbData')">
                  <span class="menu-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                      stroke-linecap="round" stroke-linejoin="round">
                      <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                      <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
                      <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
                    </svg>
                  </span>
                  <div class="menu-content">
                    <div class="menu-title">{{ $t('dataSettings.restoreDbData') }}</div>
                    <div class="menu-desc">{{ $t('dataSettings.restoreDbDataDesc') }}</div>
                  </div>
                </div>
              </el-popover>
            </div>
          </div>
          <div class="divider"></div>
          <div class="setting-row">
            <span class="row-label">{{ $t('dataSettings.resetData') }}</span>
            <div class="row-actions">
              <el-popover v-model:visible="resetVisible" trigger="click" placement="bottom-end" :width="260"
                popper-class="data-action-popover">
                <template #reference>
                  <button class="btn-action">
                    {{ $t('dataSettings.reset') }}
                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"
                      style="margin-left: 4px;">
                      <polyline points="6 9 12 15 18 9"></polyline>
                    </svg>
                  </button>
                </template>
                <div class="backup-menu-item" @click="openResetConfirm('appData')">
                  <span class="menu-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                      stroke-linecap="round" stroke-linejoin="round">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                    </svg>
                  </span>
                  <div class="menu-content">
                    <div class="menu-title">{{ $t('dataSettings.resetAppData') }}</div>
                    <div class="menu-desc">{{ $t('dataSettings.resetAppDataDesc') }}</div>
                  </div>
                </div>
                <div class="backup-menu-divider"></div>
                <div class="backup-menu-item" @click="openResetConfirm('dbData')">
                  <span class="menu-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                      stroke-linecap="round" stroke-linejoin="round">
                      <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                      <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
                      <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
                    </svg>
                  </span>
                  <div class="menu-content">
                    <div class="menu-title">{{ $t('dataSettings.resetDbData') }}</div>
                    <div class="menu-desc">{{ $t('dataSettings.resetDbDataDesc') }}</div>
                  </div>
                </div>
                <div class="backup-menu-divider"></div>
                <div class="backup-menu-item" @click="openResetConfirm('all')">
                  <span class="menu-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none"
                      stroke-linecap="round" stroke-linejoin="round">
                      <path
                        d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z">
                      </path>
                      <polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
                      <line x1="12" y1="22.08" x2="12" y2="12"></line>
                    </svg>
                  </span>
                  <div class="menu-content">
                    <div class="menu-title">{{ $t('dataSettings.resetAllData') }}</div>
                    <div class="menu-desc">{{ $t('dataSettings.resetAllDataDesc') }}</div>
                  </div>
                </div>
              </el-popover>
            </div>
          </div>
        </div>
      </div>

      <div class="data-card">
        <div class="card-header">
          <span class="card-title">{{ $t('dataSettings.dataDir') }}</span>
        </div>
        <div class="card-body">
          <div class="setting-row">
            <span class="row-label">{{ $t('dataSettings.appData') }}</span>
            <div class="row-actions">
              <span class="dir-path" @click="changePath('appData')" title="点击更改路径">{{ appDataPath }}</span>
              <button class="btn-icon" @click="openDir(appDataPath)" title="打开目录">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                  <polyline points="15 3 21 3 21 9"></polyline>
                  <line x1="10" y1="14" x2="21" y2="3"></line>
                </svg>
              </button>
            </div>
          </div>
          <div class="divider"></div>
          <div class="setting-row">
            <span class="row-label">{{ $t('dataSettings.appLog') }}</span>
            <div class="row-actions">
              <span class="dir-path" @click="changePath('appLog')" title="点击更改路径">{{ appLogPath }}</span>
              <button class="btn-icon" @click="openDir(appLogPath)" title="打开目录">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
                  <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                  <polyline points="15 3 21 3 21 9"></polyline>
                  <line x1="10" y1="14" x2="21" y2="3"></line>
                </svg>
              </button>
            </div>
          </div>
          <div class="divider"></div>
          <div class="setting-row">
            <span class="row-label">{{ $t('dataSettings.clearCache') }}</span>
            <div class="row-actions">
              <el-popover v-model:visible="clearCacheVisible" trigger="click" placement="top-end" :width="280"
                popper-class="data-action-popover">
                <template #reference>
                  <button class="btn-clear-cache">
                    {{ $t('dataSettings.clearCache') }}
                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"
                      style="margin-left: 4px;">
                      <polyline points="6 9 12 15 18 9"></polyline>
                    </svg>
                  </button>
                </template>
                <div v-for="item in cacheItems" :key="item.value">
                  <div class="backup-menu-item" @click="doClearCache(item.value)">
                    <span class="menu-icon">
                      <svg v-if="item.value === 'browser'" viewBox="0 0 24 24" width="20" height="20"
                        stroke="currentColor" stroke-width="2" fill="none">
                        <circle cx="12" cy="12" r="10"></circle>
                        <line x1="2" y1="12" x2="22" y2="12"></line>
                        <path
                          d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z">
                        </path>
                      </svg>
                      <svg v-else-if="item.value === 'db'" viewBox="0 0 24 24" width="20" height="20"
                        stroke="currentColor" stroke-width="2" fill="none">
                        <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                        <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
                        <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
                      </svg>
                      <svg v-else-if="item.value === 'app'" viewBox="0 0 24 24" width="20" height="20"
                        stroke="currentColor" stroke-width="2" fill="none">
                        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                        <polyline points="14 2 14 8 20 8"></polyline>
                      </svg>
                      <svg v-else-if="item.value === 'kg'" viewBox="0 0 24 24" width="20" height="20"
                        stroke="currentColor" stroke-width="2" fill="none">
                        <path d="M12 2L2 7l10 5 10-5-10-5z"></path>
                        <path d="M2 17l10 5 10-5"></path>
                        <path d="M2 12l10 5 10-5"></path>
                      </svg>
                      <svg v-else-if="item.value === 'all'" viewBox="0 0 24 24" width="20" height="20"
                        stroke="currentColor" stroke-width="2" fill="none">
                        <polyline points="3 6 5 6 21 6"></polyline>
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                      </svg>
                    </span>
                    <div class="menu-content">
                      <div class="menu-title">{{ item.label }}</div>
                      <div class="menu-desc">{{ item.desc }}</div>
                    </div>
                  </div>
                  <div v-if="item.value !== 'all'" class="backup-menu-divider"></div>
                </div>
              </el-popover>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- 恢复确认自定义弹窗 -->
  <div v-if="showRestoreConfirm" class="restore-confirm-overlay" @click.self="showRestoreConfirm = false">
    <div class="restore-confirm-dialog">
      <div class="restore-confirm-header">
        <h3>{{ $t('dataSettings.restoreConfirm') }}</h3>
        <button class="btn-close" @click="showRestoreConfirm = false">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
      <pre class="restore-confirm-msg">{{ restoreConfirmInfo.msg }}</pre>
      <div class="restore-confirm-actions">
        <button class="btn-cancel" @click="showRestoreConfirm = false">{{ $t('dataSettings.cancel') }}</button>
        <button class="btn-noconfirm" @click="doRestore(true)">{{ $t('dataSettings.noBackupAndRestore') }}</button>
        <button class="btn-confirm" @click="doRestore(false)">{{ $t('dataSettings.backupAndRestore') }}</button>
      </div>
    </div>
  </div>

  <!-- 重置确认自定义弹窗 -->
  <div v-if="showResetConfirm" class="restore-confirm-overlay" @click.self="showResetConfirm = false">
    <div class="restore-confirm-dialog">
      <div class="restore-confirm-header">
        <h3>{{ $t('dataSettings.resetConfirm') }}</h3>
        <button class="btn-close" @click="showResetConfirm = false">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
      <pre class="restore-confirm-msg">{{ $t('dataSettings.resetWarning', { type: resetConfirmLabel }) }}</pre>
      <div class="restore-confirm-actions">
        <button class="btn-cancel" @click="showResetConfirm = false">{{ $t('dataSettings.cancel') }}</button>
        <button class="btn-action" @click="doReset(true)">{{ $t('dataSettings.noBackupAndReset') }}</button>
        <button class="btn-confirm" @click="doReset(false)">{{ $t('dataSettings.backupAndReset') }}</button>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.data-view-container {
  display: flex;
  height: 100%;
  // background: var(--bg-primary);
  overflow: hidden;
}

.data-content {
  flex: 1;
  overflow-y: auto;
}

.data-card {
  // background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 15px;
}

.card-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;

  .card-title {
    font-size: var(--font-size-base);
    font-weight: 600;
    color: var(--text-primary);
  }
}

.card-body {
  padding: 0;
}

.setting-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 20px;
  min-height: 64px;
  box-sizing: border-box;

  .row-label {
    font-size: var(--font-size-base);
    color: var(--text-primary);
  }
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 8px;

  .dir-path {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
    font-family: monospace;
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;

    &:hover {
      color: var(--color-primary);
    }
  }
}

.btn-action {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 16px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  font-size: var(--font-size-base);
  cursor: pointer;
  transition: all 0.18s ease;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
    opacity: 0.9;
  }
}

.btn-icon {
  padding: 4px;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;

  &:hover {
    background: var(--bg-hover);
    color: var(--color-primary);
  }
}

.btn-clear-cache {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 16px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  font-size: var(--font-size-base);
  cursor: pointer;
  transition: all 0.18s ease;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
    opacity: 0.9;
  }
}

.divider {
  height: 1px;
  background: var(--border-color);
  margin: 0 20px;
}

// el-popover 弹出层内边距覆盖
:global(.data-action-popover) {
  padding: 8px 0 !important;
}

// 情景主题下 popover 背景样式
:global(.light[data-theme-id^="scene-"] .data-action-popover),
:global(.dark[data-theme-id^="scene-"] .data-action-popover) {
  background: var(--theme-dailog-color) !important;
  border-color: var(--border-color) !important;
}

.backup-menu-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 16px;
  cursor: pointer;
  transition: background 0.18s ease;

  &:hover {
    background: var(--bg-hover);
  }

  .menu-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    color: var(--text-secondary);
  }

  .menu-content {
    flex: 1;
  }

  .menu-title {
    font-size: var(--font-size-base);
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: 4px;
  }

  .menu-desc {
    font-size: var(--font-size-sm);
    color: var(--text-secondary);
  }
}

.backup-menu-divider {
  height: 1px;
  background: var(--border-color);
  margin: 0;
}

// 确保备份成功/失败弹窗显示在最上层
:global(.backup-success-dialog),
:global(.backup-error-dialog) {
  z-index: 99999 !important;
}

:global(.backup-success-dialog .el-message-box__wrapper),
:global(.backup-error-dialog .el-message-box__wrapper) {
  z-index: 99999 !important;
}

:global(.light[data-theme-id^="scene-"] .backup-menu-item .menu-title),
:global(.dark[data-theme-id^="scene-"] .backup-menu-item .menu-title) {
  color: var(--text-primary) !important;
}

:global(.light[data-theme-id^="scene-"] .backup-menu-item .menu-desc),
:global(.dark[data-theme-id^="scene-"] .backup-menu-item .menu-desc) {
  color: var(--text-secondary) !important;
}

:global(.light[data-theme-id^="scene-"] .backup-menu-item:hover),
:global(.dark[data-theme-id^="scene-"] .backup-menu-item:hover) {
  background: var(--bg-hover) !important;
}

:global(.light[data-theme-id^="scene-"] .backup-menu-divider),
:global(.dark[data-theme-id^="scene-"] .backup-menu-divider) {
  background: var(--border-color) !important;
}

.restore-confirm-overlay {
  position: fixed;
  inset: 0;
  z-index: 100000;
  background: rgba(15, 23, 42, 0.36);
  backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  justify-content: center;
}

.restore-confirm-dialog {
  background: var(--theme-dailog-color);
  border-radius: 8px;
  padding: 24px;
  width: 440px;
  max-width: 90vw;
  border: 1px solid var(--border-color);
  color: var(--text-primary);
}

.restore-confirm-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin: 0 0 16px;

  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .btn-close {
    padding: 4px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;

    &:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
}

.restore-confirm-msg {
  white-space: pre-wrap;
  font-size: var(--font-size-base);
  color: var(--text-secondary);
  margin: 0 0 24px;
  line-height: 1.6;
}

.restore-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;

  button {
    padding: 8px 16px;
    border-radius: 4px;
    font-size: var(--font-size-base);
    cursor: pointer;
    transition: all 0.2s;
    border: 1px solid transparent;
  }

  .btn-cancel {
    background: transparent;
    border-color: var(--border-color);
    color: var(--text-secondary);

    &:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }

  .btn-noconfirm {
    background: var(--color-warning, #faad14);
    color: #fff;

    &:hover {
      background: var(--color-warning-hover, #d48806);
    }
  }

  .btn-confirm {
    background: var(--color-primary);
    color: #fff;

    &:hover {
      background: var(--color-primary-hover);
    }
  }
}
</style>

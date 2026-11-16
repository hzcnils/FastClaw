// FastClaw · WSP 服务管理状态
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { WspService, WspEventLog } from '@/types'

export const useWspStore = defineStore('wsp', () => {
  const services = ref<WspService[]>([])
  const isLoading = ref(false)

  async function loadServices() {
    isLoading.value = true
    try {
      const result = await invoke<WspService[]>('list_wsp_services')
      services.value = result || []
    } catch (err) {
      console.error('[WSP Store] Error loading services:', err)
    } finally {
      isLoading.value = false
    }
  }

  //启动服务
  async function startService(serviceId: string) {
    try {
      await invoke('start_wsp_service', { serviceId });
      await loadServices();
    } catch (err) {
      console.error(`[WSP] 启动失败:`, err);
      await loadServices();  // 失败后也重新加载，获取错误状态
      throw err              // 让调用方能拿到后端的具体错误信息
    }
  }

  async function stopService(serviceId: string) {
    await invoke('stop_wsp_service', { serviceId })
    await loadServices()
  }

  async function restartService(serviceId: string) {
    await invoke('restart_wsp_service', { serviceId })
    await loadServices()
  }

  async function startAll() {
    try {
      await invoke('start_all_wsp_services')
      await loadServices()
    } catch (err) {
      console.error('[WSP] 全部启动失败:', err)
      alert('全部启动失败，请查看控制台获取详细错误信息')
    }
  }

  async function stopAll() {
    await invoke('stop_all_wsp_services')
    await loadServices()
  }

  async function getLogs(serviceId: string, limit?: number): Promise<WspEventLog[]> {
    return await invoke('get_wsp_logs', { serviceId, limit })
  }

  async function callEndpoint(serviceId: string, method: string, path: string, body?: any) {
    return await invoke('call_wsp_endpoint', { serviceId, method, path, body })
  }

  async function registerService(input: any) {
    await invoke('register_wsp_service', { input })
    await loadServices()
  }

  async function removeService(serviceId: string) {
    try {
      await invoke('remove_wsp_service', { serviceId })
      await loadServices()
    } catch (err) {
      console.error('[WSP] Failed to remove service:', err)
      throw err
    }
  }

  async function editService(params: {
    serviceId: string
    displayName: string
    description?: string
    exePath: string
    port: number
    autoStart: boolean
    category?: string
    categoryName?: string
    subCategoryName?: string
    host?: string
    basePath?: string
  }) {
    await invoke('edit_wsp_service', {
      serviceId: params.serviceId,
      displayName: params.displayName,
      description: params.description,
      exePath: params.exePath,
      port: params.port,
      autoStart: params.autoStart,
      category: params.category,
      categoryName: params.categoryName,
      subCategoryName: params.subCategoryName,
      host: params.host,
      basePath: params.basePath,
    })
    await loadServices()
  }

  /**
   * 上传 WSP 服务包并注册
   * @param filePath - 文件路径
   * @param category - 服务分类
   * @param port - 端口号
   * @param displayName - 显示名称（可选）
   * @returns 创建的服务信息
   */
  async function uploadWspService(filePath: string, category: string, port: number, displayName?: string): Promise<WspService> {
    console.log('[WSP] 上传服务:', { filePath, category, port, displayName });

    const service = await invoke<WspService>('upload_wsp_service_by_path', {
      filePath,
      category,
      port,
      displayName,
    });

    console.log('[WSP] 服务上传成功:', service);
    await loadServices();
    return service;
  }

  return {
    services, isLoading,
    loadServices, startService, stopService, restartService,
    startAll, stopAll, getLogs, callEndpoint,
    registerService, removeService, editService, uploadWspService
  }
})

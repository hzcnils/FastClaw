import { normalizeFastPlanPath } from './inputSlots.js'
import type { FastPlanAccess } from './access'

export interface FastPlanUploadResult {
  path: string
  raw: unknown
}

export interface FastPlanUploadOptions {
  access?: FastPlanAccess
  apiBase?: string
  XMLHttpRequest?: typeof XMLHttpRequest
  onProgress?: (progress: number) => void
}

/** Upload a single FastPlan attachment using the backend's stable `/api/upload` contract. */
export function uploadFastPlanFile(file: File, options: FastPlanUploadOptions = {}) {
  const Xhr = options.XMLHttpRequest || XMLHttpRequest
  const apiBase = (options.apiBase || '').replace(/\/$/, '')

  const xhr = new Xhr()
  let cancelled = false
  const abort = xhr.abort.bind(xhr)
  // Abort before the asynchronous token lookup completes must also cancel the upload.
  xhr.abort = () => { cancelled = true; abort() }
  const promise = new Promise<FastPlanUploadResult>((resolve, reject) => {
    const data = new FormData()
    data.append('file', file)
    xhr.open('POST', `${apiBase}/api/upload`)
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable && event.total > 0) {
        options.onProgress?.(Math.min(99, (event.loaded / event.total) * 100))
      } else if (event.loaded > 0) {
        options.onProgress?.(10)
      }
    }
    xhr.upload.onload = () => options.onProgress?.(99)
    xhr.onload = () => {
      if (xhr.status < 200 || xhr.status >= 300) {
        reject(new Error(`HTTP ${xhr.status}`))
        return
      }
      try {
        const raw = JSON.parse(xhr.responseText)
        const path = normalizeFastPlanPath(raw?.path)
        if (!path) throw new Error('上传响应缺少文件路径')
        resolve({ path, raw })
      } catch (error) {
        reject(error instanceof Error ? error : new Error('返回解析失败'))
      }
    }
    xhr.onerror = () => reject(new Error('网络错误'))
    xhr.onabort = () => reject(new Error('上传已取消'))
    if (options.access) {
      options.access.getConnection().then(connection => {
        if (cancelled) throw new Error('上传已取消')
        const target = new URL('/api/upload', connection.apiBase)
        if (!connection.parentToken || target.origin !== new URL(connection.apiBase).origin || target.username || target.password) throw new Error('FastPlan 上传地址与当前托管服务不一致')
        xhr.open('POST', target.href)
        xhr.setRequestHeader('Authorization', `Bearer ${connection.parentToken}`)
        xhr.send(data)
      }).catch(reject)
    } else xhr.send(data)
  })
  return { xhr, promise }
}

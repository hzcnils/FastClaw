/**
 * Axios HTTP client — general-purpose instance.
 *
 * baseURL 默认 http://127.0.0.1:5179（axum 默认端口），
 * 首次请求时通过 invoke('get_http_port') 获取实际端口（应对端口漂移）。
 */
import axios from 'axios'

const http = axios.create({
  baseURL: 'http://127.0.0.1:5179',
  timeout: 30000,
  headers: { 'Content-Type': 'application/json' },
})

let portFetched = false

async function updateBaseURL() {
  if (portFetched) return
  portFetched = true
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const port = await invoke<number>('get_http_port')
    if (port && port > 0 && port !== 5179) {
      http.defaults.baseURL = `http://127.0.0.1:${port}`
    }
  } catch {
    // 非 Tauri 环境 — 使用相对路径
    http.defaults.baseURL = ''
  }
}

http.interceptors.request.use(async (config) => {
  await updateBaseURL()
  // 发布版没有登录。用量看板等 /newApi/ 请求固定使用管理员身份，不读取旧登录令牌。
  if (config.url && config.url.indexOf('/newApi/') > -1) {
    config.headers['new-api-user'] = '1'
  }
  return config
})

http.interceptors.response.use(
  (response) => response.data,
  (error) => {
    console.error('API Error:', error.message)
    return Promise.reject(error)
  },
)

export default http

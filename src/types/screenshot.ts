/** 显示器信息 */
export interface MonitorInfo {
  x: number
  y: number
  width: number
  height: number
  is_primary: boolean
  name: string
}

/** 窗口信息 */
export interface WindowInfo {
  handle: number
  x: number
  y: number
  width: number
  height: number
  title: string
  is_foreground: boolean
}

/** Rust 后端 capture_fullscreen_with_windows 返回结果（旧接口，含 base64） */
export interface CaptureResult {
  /** JPEG base64 data URL (quality 95) */
  base64_data: string
  width: number
  height: number
  offset_x: number
  offset_y: number
  monitors: MonitorInfo[]
  windows: WindowInfo[]
}

/** 两步 IPC Step 1 的元数据（capture_start 返回，不含像素） */
export interface CaptureMetadata {
  /** Origin request, echoed by the native overlay. */
  request_id?: string | null
  presentation?: 'window' | 'inline' | null
  width: number
  height: number
  offset_x: number
  offset_y: number
  monitors: MonitorInfo[]
  windows: WindowInfo[]
  /** 每个显示器的 JPEG base64 data URL（与 monitors 顺序对应），前端调用 capture_get_all_monitor_pixels 后填充 */
  monitorImages?: string[]
}

/** 截图模式 */
export type ScreenshotMode = 'free' | 'active_window' | 'specified_window'
export interface ScreenshotReply { requestId: string; dataUrl?: string }

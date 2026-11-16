/**
 * 前端业务配置类型定义
 *
 * 菜单/设置页显隐配置已统一迁移到 exe 同目录 config.json（Rust ConfigManager 管理），
 * 通过 invoke('get_app_config') 加载，不再从 public/config.json 静态读取。
 * 本文件仅保留类型定义和默认值常量，供 appConfig Store 和业务配置设置页引用。
 *
 * 兜底原则：配置缺失/损坏时回退到全部可见的默认行为，不引入线上故障。
 */

/** 前端业务配置 */
export interface AppFrontendConfig {
  /** 菜单可见性，key 为菜单项 id，默认全部 true */
  menus: Record<string, boolean>
  /** 设置页可见性，key 为设置页 key，默认全部 true */
  settings: Record<string, boolean>
}

/** 所有菜单项 id 的完整列表 */
export const ALL_MENU_IDS = [
  'chat',
  'kb',
  'models',
  'alltool',
  'skills',
  'mcp',
  'wsp',
  'dashboard',
] as const

/** 菜单项中文标签映射 */
export const MENU_LABELS: Record<string, string> = {
  chat: '智能对话',
  kb: '知识管理',
  models: '模型管理',
  alltool: '全部工具',
  skills: 'Skills管理',
  mcp: 'MCP服务',
  wsp: 'WSP管理',
  dashboard: '调用看板',
}

/** 所有设置页 key 的完整列表 */
export const ALL_SETTING_KEYS = [
  'settings',
  'defaultModel',
  'networkService',
  'data',
  'personalAssistant',
  'pluginMarket',
  'webSearch',
  'aboutus',
  'businessConfig',
] as const

/** 设置页中文标签映射 */
export const SETTING_LABELS: Record<string, string> = {
  settings: '参数设置',
  defaultModel: '默认模型',
  networkService: '网络服务',
  data: '数据设置',
  personalAssistant: '个人助理',
  pluginMarket: '插件市场',
  webSearch: '网络搜索',
  aboutus: '关于我们',
  businessConfig: '业务配置',
}

/** 默认配置（全部可见） */
export const DEFAULT_FRONTEND_CONFIG: AppFrontendConfig = {
  menus: Object.fromEntries(ALL_MENU_IDS.map((id) => [id, true])) as Record<string, boolean>,
  settings: Object.fromEntries(ALL_SETTING_KEYS.map((key) => [key, true])) as Record<string, boolean>,
}

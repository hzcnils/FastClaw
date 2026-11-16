// FastClaw · 模型选择器共享工具

import { isHiddenReleaseProvider } from '@/utils/provider'

// ---- 标签解析 ----

/** 将 modelLabel 逗号分隔字符串解析为标签数组 */
export function parseTags(modelLabel?: string): string[] {
  if (!modelLabel) return []
  return modelLabel.split(',').map(t => t.trim()).filter(Boolean)
}

// ---- 预设标签 → 图标映射 ----

export interface TagObject {
  value: string
  label: string
}

const PRESET_TAG_ICONS: Record<string, string> = {
  '视觉': 'image',
  '联网': 'internet',
  '推理': 'think',
  '工具': 'TOOL_EXECUTION',
}

/** 将 string[] 转为 TagObject[]，已知标签绑定图标，未知标签仅显示文本 */
export function toTagObjects(tags: string[]): TagObject[] {
  return tags.map(t => {
    const icon = PRESET_TAG_ICONS[t]
    return icon ? { value: icon, label: t } : { value: '', label: t }
  })
}

// ---- 置顶持久化 ----

const PINNED_MODELS_KEY = 'fastclaw-pinned-models'

export function getPinnedModelIds(): string[] {
  try {
    const raw = localStorage.getItem(PINNED_MODELS_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

export function setPinnedModelIds(ids: string[]): void {
  localStorage.setItem(PINNED_MODELS_KEY, JSON.stringify(ids))
}

export function addPinnedModelId(id: string): void {
  const current = getPinnedModelIds()
  if (!current.includes(id)) {
    setPinnedModelIds([...current, id])
  }
}

export function removePinnedModelId(id: string): void {
  setPinnedModelIds(getPinnedModelIds().filter(x => x !== id))
}

// ---- 数据类型 ----

export interface MergedModel {
  modelId: number | string
  modelName: string
  modelLabel: string
  tags: string[]
}

export interface MergedPlatform {
  platformId: number | string
  platformName: string
  providerCode?: string
  sourceCategory?: string
  modelList: MergedModel[]
}

export interface SelectModelItem {
  modelId: number | string
  modelName: string
  modelLabel: string
  tags: string[]
  show: boolean
  oldIndex?: [number, number]
  modelImg?: string
}

export interface SelectModelGroup {
  label: string
  alwaysShow?: boolean
  providerCode?: string
  sourceCategory?: string
  children: SelectModelItem[]
}

// ---- 数据转换 ----

/** 将 list_models_tree 的原始数据转换为带 tags 的格式 */
export function enrichTreeData(tree: any[]): MergedPlatform[] {
  return tree.filter((platform: any) => !isHiddenReleaseProvider(
    platform.providerCode || platform.provider_code || (platform.platformId === 'local' ? 'LOCAL' : ''),
    platform.platformId,
  )).map((platform: any) => ({
    platformId: platform.platformId,
    platformName: platform.platformName,
    providerCode: platform.providerCode || platform.provider_code || (platform.platformId === 'local' ? 'LOCAL' : ''),
    sourceCategory: platform.sourceCategory || platform.source_category || 'online',
    modelList: (platform.modelList || []).map((m: any) => {
      const modelLabel = m.modelLabel || m.model_label || ''
      return {
        modelId: m.modelId,
        modelName: m.modelName || '',
        modelLabel,
        tags: parseTags(modelLabel),
      }
    }),
  }))
}

/** list_models_tree 不可用时，将 flat list_models 数据按 provider 分组 */
export function groupFlatModels(flat: any[]): MergedPlatform[] {
  const enabled = flat.filter((m: any) => {
    if (m.is_enabled === false) return false
    const code = m.provider_code
    if (!code) return false
    return !isHiddenReleaseProvider(code)
  })
  const groupMap = new Map<string, MergedModel[]>()
  for (const m of enabled) {
    const key = m.provider_name || '默认'
    const modelLabel = m.model_label || ''
    if (!groupMap.has(key)) groupMap.set(key, [])
    groupMap.get(key)!.push({
      modelId: m.id,
      modelName: m.display_name || m.model_id || '',
      modelLabel,
      tags: parseTags(modelLabel),
    })
  }
  return Array.from(groupMap.entries()).map(([name, models], i) => ({
    platformId: i,
    platformName: name,
    modelList: models,
  }))
}

/** 将 MergedPlatform[] 转为 selectModel.vue 所需的分组格式（含置顶分组） */
export function transformToSelectModelGroups(
  platforms: MergedPlatform[],
  pinnedIds: string[],
): SelectModelGroup[] {
  const pinnedItems: SelectModelItem[] = []

  const groups: SelectModelGroup[] = platforms.map((p, gi) => ({
    label: p.platformName,
    providerCode: p.providerCode,
    sourceCategory: p.sourceCategory,
    children: p.modelList
      .map((m, ci) => {
        const item: SelectModelItem = {
          modelId: m.modelId,
          modelName: m.modelName,
          modelLabel: m.modelLabel,
          tags: m.tags,
          show: true,
          oldIndex: [gi, ci],
        }
        if (pinnedIds.includes(String(m.modelId))) {
          item.show = false
          pinnedItems.push({ ...item, show: true })
        }
        return item
      }),
  }))

  // 过滤空分组，插入置顶分组
  const filtered = groups.filter(g => g.children.length > 0)
  if (pinnedItems.length > 0) {
    filtered.unshift({
      label: '已置顶',
      alwaysShow: true,
      children: pinnedItems,
    })
  }
  return filtered
}

/** Whether a provider is the built-in Huizhi cloud provider. */
export function isHuizhiProvider(providerCode?: string | null): boolean {
  return String(providerCode || '').trim().toUpperCase() === 'HUIZHI'
}

/** 公开版对话和默认模型选择只保留互联网供应商。 */
const HIDDEN_RELEASE_PROVIDER_CODES = new Set(['HUIZHI', 'PRIVATE', 'LOCAL', 'OLLAMA', 'LLAMACPP'])

export function isHiddenReleaseProvider(providerCode?: string | null, platformId?: string | null): boolean {
  const code = String(providerCode || '').trim().toUpperCase()
  if (HIDDEN_RELEASE_PROVIDER_CODES.has(code)) return true
  return String(platformId || '').trim().toLowerCase() === 'local'
}

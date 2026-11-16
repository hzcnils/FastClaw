// Billing render functions for useLog dashboard

function formatRatio(r: any): string {
  if (r === undefined || r === null) return '-'
  if (typeof r === 'number') return r.toFixed(4)
  return String(r)
}

function renderQuotaLocal(quota: number, digits = 2): string {
  const quotaPerUnit = parseFloat(localStorage.getItem('newapi_quota_per_unit') || '500000')
  const quotaDisplayType = localStorage.getItem('newapi_quota_display_type') || 'USD'
  if (quotaDisplayType === 'TOKENS') return String(quota)
  const resultUSD = quota / quotaPerUnit
  let symbol = '$'
  let value = resultUSD
  if (quotaDisplayType === 'CNY') { value = resultUSD; symbol = '¥' }
  return symbol + value.toFixed(digits)
}

function renderNumber(num: number): string {
  if (num >= 1000000000) return (num / 1000000000).toFixed(1) + 'B'
  if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M'
  if (num >= 10000) return (num / 1000).toFixed(1) + 'k'
  return String(num)
}

export function renderClaudeLogContent(
  modelRatio: any, completionRatio: any, modelPrice: any, groupRatio: any,
  userGroupRatio: any, cacheRatio: any, cacheCreationRatio: any,
  cacheCreationTokens5m: any, cacheCreationRatio5m: any,
  cacheCreationTokens1h: any, cacheCreationRatio1h: any,
  displayMode: string,
): string {
  return `Model: ${formatRatio(modelRatio)}x | Group: ${formatRatio(groupRatio)}x`
}

export function renderLogContent(
  modelRatio: any, completionRatio: any, modelPrice: any, groupRatio: any,
  userGroupRatio: any, cacheRatio: any, _split: boolean, _imageRatio: any,
  webSearch: boolean, webSearchCallCount: number,
  fileSearch: boolean, fileSearchCallCount: number,
  displayMode: string,
): string {
  return `Model: ${formatRatio(modelRatio)}x | Group: ${formatRatio(groupRatio)}x`
}

export function renderAudioModelPrice(
  textInput: any, textOutput: any, modelRatio: any, modelPrice: any,
  completionRatio: any, audioInput: any, audioOutput: any,
  audioRatio: any, audioCompletionRatio: any, groupRatio: any,
  userGroupRatio: any, cacheTokens: number, cacheRatio: any,
  displayMode: string,
): string {
  return `Model: ${formatRatio(modelRatio)}x | Audio: ${formatRatio(audioRatio)}x`
}

export function renderClaudeModelPrice(
  promptTokens: number, completionTokens: number,
  modelRatio: any, modelPrice: any, completionRatio: any,
  groupRatio: any, userGroupRatio: any,
  cacheTokens: number, cacheRatio: any,
  cacheCreationTokens: number, cacheCreationRatio: any,
  cacheCreationTokens5m: number, cacheCreationRatio5m: any,
  cacheCreationTokens1h: number, cacheCreationRatio1h: any,
  displayMode: string,
): string {
  return `Prompt: ${renderNumber(promptTokens)} | Completion: ${renderNumber(completionTokens)}`
}

export function renderModelPrice(
  promptTokens: number, completionTokens: number,
  modelRatio: any, modelPrice: any, completionRatio: any,
  groupRatio: any, userGroupRatio: any,
  cacheTokens: number, cacheRatio: any,
  isImage: boolean, imageRatio: any, imageOutput: number,
  webSearch: boolean, webSearchCallCount: number, webSearchPrice: any,
  fileSearch: boolean, fileSearchCallCount: number, fileSearchPrice: any,
  audioInputSeparate: boolean, audioInputTokenCount: number, audioInputPrice: any,
  imageGenCall: boolean, imageGenCallPrice: any,
  displayMode: string,
): string {
  return `Prompt: ${renderNumber(promptTokens)} | Completion: ${renderNumber(completionTokens)}`
}

export function renderModelPriceSimple(
  modelRatio: any, modelPrice: any, groupRatio: any, userGroupRatio: any,
  cacheTokens: number, cacheRatio: any,
  cacheCreationTokens: number, cacheCreationRatio: any,
  cacheCreationTokens5m: number, cacheCreationRatio5m: any,
  cacheCreationTokens1h: number, cacheCreationRatio1h: any,
  isSplit: boolean, imageRatio: any,
  isSystemPromptOverwritten: any, modelType: string,
  displayMode: string, mode: string = 'segments',
): any {
  if (mode === 'segments') {
    return {
      segments: [
        { text: `${formatRatio(modelRatio)}x`, tone: 'primary' },
        { text: renderQuotaLocal(0, 6), tone: 'secondary' },
      ],
    }
  }
  return `Model: ${formatRatio(modelRatio)}x`
}

export interface FastPlanServiceConfig {
  apiBase: string
  wsBase: string
  uploadBase: string
  downloadBase: string
  adminBase: string
}

export interface LocationOriginLike {
  protocol: string
  host: string
}

function withoutTrailingSlash(value: string): string {
  return value.trim().replace(/\/+$/, '')
}

export function toFastPlanWebSocketBase(
  serviceUrl: string,
  locationOrigin?: LocationOriginLike,
): string {
  const normalized = withoutTrailingSlash(serviceUrl)
  if (/^https?:\/\//i.test(normalized)) {
    return normalized.replace(/^http:/i, 'ws:').replace(/^https:/i, 'wss:')
  }
  if (/^wss?:\/\//i.test(normalized)) return normalized
  if (normalized.startsWith('/') && locationOrigin) {
    const protocol = locationOrigin.protocol === 'https:' ? 'wss:' : 'ws:'
    return `${protocol}//${locationOrigin.host}${normalized}`
  }
  return normalized
}

export function resolveFastPlanServiceConfig(
  serviceUrl: string,
  locationOrigin?: LocationOriginLike,
): FastPlanServiceConfig {
  const apiBase = withoutTrailingSlash(serviceUrl)
  return {
    apiBase,
    wsBase: toFastPlanWebSocketBase(apiBase, locationOrigin),
    uploadBase: apiBase,
    downloadBase: apiBase,
    adminBase: apiBase,
  }
}

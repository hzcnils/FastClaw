import { invoke } from '@tauri-apps/api/core'

export type FastKgDownloadSourceType = 'public' | 'private'

export interface FastKgDownloadSource {
  type: FastKgDownloadSourceType
  baseUrl: string
}

export interface FastKgDownloadConnectivity {
  internetReachable: boolean
  privateReachable: boolean
  source: FastKgDownloadSource | null
}

export interface FastKgDownloadConnectivityOptions {
  internetCheckUrl: string
  publicBaseUrl: string
  privateBaseUrl: string
}

function normalizeBaseUrl(value: string): string {
  return value.trim().replace(/\/+$/, '')
}

async function probeUrl(url: string): Promise<boolean> {
  if (!url) return false
  try {
    return await invoke<boolean>('check_internet_connectivity', { url })
  } catch {
    return false
  }
}

/**
 * Resolve a usable FastKG download source without changing the global
 * internet-connectivity state. Public internet and the configured private
 * download service are probed independently so an intranet-only deployment
 * can still download the service package.
 */
export async function resolveFastKgDownloadSource(
  options: FastKgDownloadConnectivityOptions,
): Promise<FastKgDownloadConnectivity> {
  const publicBaseUrl = normalizeBaseUrl(options.publicBaseUrl)
  const privateBaseUrl = normalizeBaseUrl(options.privateBaseUrl)
  const privateProbeUrl = privateBaseUrl
    ? `${privateBaseUrl}/api/v1/apps/FastKG`
    : ''

  const [internetReachable, privateReachable] = await Promise.all([
    probeUrl(options.internetCheckUrl.trim()),
    probeUrl(privateProbeUrl),
  ])

  const source = internetReachable && publicBaseUrl
    ? { type: 'public' as const, baseUrl: publicBaseUrl }
    : privateReachable && privateBaseUrl
      ? { type: 'private' as const, baseUrl: privateBaseUrl }
      : null

  return { internetReachable, privateReachable, source }
}

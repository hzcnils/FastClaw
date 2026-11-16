/** Format milliseconds for both the live clock and reported backend durations. */
export function formatDuration(milliseconds, { live = false } = {}) {
  if (milliseconds == null || milliseconds === '') return ''
  const ms = Number(milliseconds)
  if (!Number.isFinite(ms) || ms < 0) return ''
  if (!live && Math.round(ms) < 1000) return `${Math.round(ms)}ms`
  const seconds = live ? Math.floor(ms / 1000) : Math.round(ms / 100) / 10
  if (seconds < 60) return `${seconds}s`
  const total = Math.floor(seconds)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor(total / 60) % 60
  const remainder = total % 60
  return hours ? `${hours}h ${minutes}m ${remainder}s` : `${minutes}m ${remainder}s`
}

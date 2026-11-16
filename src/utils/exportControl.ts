/** Optional caller ownership. Once a filesystem write starts it cannot be retracted. */
export interface ExportControl { isCurrent(): boolean }
export function assertExportCurrent(control?: ExportControl) {
  if (control && !control.isCurrent()) throw new DOMException('导出请求已失效', 'AbortError')
}

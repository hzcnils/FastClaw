export const CHAT_PLAN_PACKAGES_FLAG = 'fastclaw.chatPlanPackages.enabled'

export function isChatPlanPackagesEnabled(storage?: Pick<Storage, 'getItem'>): boolean {
  const source = storage || (typeof localStorage !== 'undefined' ? localStorage : undefined)
  return source?.getItem(CHAT_PLAN_PACKAGES_FLAG) !== 'false'
}

export function setChatPlanPackagesEnabled(enabled: boolean, storage?: Pick<Storage, 'setItem'>): void {
  const target = storage || (typeof localStorage !== 'undefined' ? localStorage : undefined)
  target?.setItem(CHAT_PLAN_PACKAGES_FLAG, String(enabled))
}


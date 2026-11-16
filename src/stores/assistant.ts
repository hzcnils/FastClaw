import { ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import type {
  Role,
  RoleStats,
  RoleFilter,
  SquareRole,
  SquareFilter,
  CreateRoleData,
  PagedResult,
} from '@/types/assistant'

export const useAssistantStore = defineStore('assistant', () => {
  const roles = ref<Role[]>([])
  const activeRole = ref<Role | null>(null)
  const squareRoles = ref<SquareRole[]>([])
  const squareTotal = ref(0)
  const stats = ref<RoleStats>({ total: 0, published: 0, activeName: '无' })
  const loading = ref(false)
  const squareLoading = ref(false)
  let rolesLoaded = false
  let rolesLoadPromise: Promise<void> | null = null
  let activeRoleLoaded = false
  let activeRoleLoadPromise: Promise<void> | null = null
  let roleEventUnlisten: (() => void) | null = null
  let roleEventListenPromise: Promise<void> | null = null

  const filter = ref<RoleFilter>({ search: '', sourceType: '', isActive: '' })
  const squareFilter = ref<SquareFilter>({
    search: '',
    category: 'all',
    sortBy: 'newest',
    page: 1,
    pageSize: 9,
  })

  async function performLoadRoles() {
    loading.value = true
    try {
      const f = filter.value
      const params: Record<string, unknown> = {
        search: f.search || null,
        sourceType: f.sourceType || null,
        isActive: f.isActive === 'true' ? true : f.isActive === 'false' ? false : null,
      }
      roles.value = await invoke<Role[]>('get_roles', params)
    } catch (e) {
      console.error('loadRoles failed:', e)
    } finally {
      loading.value = false
    }
  }

  async function loadStats() {
    try {
      stats.value = await invoke<RoleStats>('get_role_stats')
    } catch (e) {
      console.error('loadStats failed:', e)
    }
  }

  async function performLoadActiveRole() {
    try {
      activeRole.value = await invoke<Role | null>('get_active_role') ?? null
    } catch (e) {
      console.error('loadActiveRole failed:', e)
    }
  }

  function runRolesLoad(force: boolean): Promise<void> {
    if (!force && rolesLoaded) return Promise.resolve()
    if (rolesLoadPromise) return rolesLoadPromise
    rolesLoadPromise = performLoadRoles()
      .then(() => { rolesLoaded = true })
      .finally(() => { rolesLoadPromise = null })
    return rolesLoadPromise
  }

  function runActiveRoleLoad(force: boolean): Promise<void> {
    if (!force && activeRoleLoaded) return Promise.resolve()
    if (activeRoleLoadPromise) return activeRoleLoadPromise
    activeRoleLoadPromise = performLoadActiveRole()
      .then(() => { activeRoleLoaded = true })
      .finally(() => { activeRoleLoadPromise = null })
    return activeRoleLoadPromise
  }

  function loadRoles() { return runRolesLoad(true) }
  function ensureRolesLoaded() { return runRolesLoad(false) }
  function loadActiveRole() { return runActiveRoleLoad(true) }
  function ensureActiveRoleLoaded() { return runActiveRoleLoad(false) }

  /** The assistant store is application-scoped, so only one backend listener is needed. */
  function ensureRoleEventListener(): Promise<void> {
    if (roleEventUnlisten) return Promise.resolve()
    if (roleEventListenPromise) return roleEventListenPromise
    roleEventListenPromise = import('@tauri-apps/api/event')
      .then(({ listen }) => listen<string>('role-activated', () => { void loadActiveRole() }))
      .then((unlisten) => { roleEventUnlisten = unlisten })
      .catch((error) => { console.warn('listen role-activated failed:', error) })
      .finally(() => { roleEventListenPromise = null })
    return roleEventListenPromise
  }

  async function createRole(data: CreateRoleData): Promise<Role | null> {
    try {
      const role = await invoke<Role>('create_role', { data })
      await loadRoles()
      await loadStats()
      ElMessage.success('角色创建成功')
      return role
    } catch (e: any) {
      ElMessage.error(e?.message || '创建失败，请稍后重试')
      return null
    }
  }

  async function updateRole(id: string, data: CreateRoleData): Promise<Role | null> {
    try {
      const role = await invoke<Role>('update_role', { id, data })
      await loadRoles()
      ElMessage.success('角色保存成功')
      return role
    } catch (e: any) {
      ElMessage.error(e?.message || '保存失败，请稍后重试')
      return null
    }
  }

  async function deleteRole(id: string): Promise<boolean> {
    try {
      await invoke('delete_role', { id })
      if (activeRole.value?.id === id) {
        activeRole.value = null
      }
      await loadRoles()
      await loadStats()
      ElMessage.success('角色已删除')
      return true
    } catch (e: any) {
      ElMessage.error(e?.message || '删除失败，请稍后重试')
      return false
    }
  }

  async function activateRole(id: string): Promise<boolean> {
    try {
      await invoke('activate_role', { id })
      await loadActiveRole()
      await loadRoles()
      const name = roles.value.find(r => r.id === id)?.name ?? ''
      ElMessage.success(`角色 ${name} 已激活`)
      return true
    } catch (e: any) {
      ElMessage.error(e?.message || '激活失败')
      return false
    }
  }

  async function deactivateRole(): Promise<void> {
    try {
      await invoke('deactivate_role')
      activeRole.value = null
      await loadRoles()
    } catch (e) {
      console.error('deactivateRole failed:', e)
    }
  }

  async function publishRole(id: string, category: string, squareDesc: string): Promise<boolean> {
    try {
      await invoke('publish_role', { id, category, squareDesc })
      await loadRoles()
      ElMessage.success('角色已发布到广场')
      return true
    } catch (e: any) {
      ElMessage.error(e?.message || '发布失败')
      return false
    }
  }

  async function unpublishRole(id: string): Promise<boolean> {
    try {
      await invoke('unpublish_role', { id })
      await loadRoles()
      ElMessage.info('角色已从广场撤回')
      return true
    } catch (e: any) {
      ElMessage.error(e?.message || '撤回失败')
      return false
    }
  }

  async function cloneRole(id: string): Promise<boolean> {
    try {
      await invoke('clone_role', { id })
      await loadRoles()
      await loadStats()
      ElMessage.success('角色已复制')
      return true
    } catch (e: any) {
      ElMessage.error(e?.message || '复制失败')
      return false
    }
  }

  async function loadSquareRoles() {
    squareLoading.value = true
    try {
      const f = squareFilter.value
      const result = await invoke<PagedResult<SquareRole>>('list_square_roles', {
        search: f.search || null,
        category: f.category === 'all' ? null : f.category,
        sortBy: f.sortBy,
        page: f.page,
        pageSize: f.pageSize,
      })
      squareRoles.value = result.items
      squareTotal.value = result.total
    } catch (e) {
      console.error('loadSquareRoles failed:', e)
    } finally {
      squareLoading.value = false
    }
  }

  async function applySquareRole(squareId: string): Promise<Role | null> {
    try {
      const role = await invoke<Role>('apply_square_role', { squareId })
      await loadRoles()
      await loadStats()
      return role
    } catch (e: any) {
      ElMessage.error(e?.message || '应用失败')
      return null
    }
  }

  async function starSquareRole(squareId: string): Promise<void> {
    try {
      await invoke('star_square_role', { squareId })
      const item = squareRoles.value.find(r => r.id === squareId)
      if (item) item.starCount++
    } catch (e) {
      console.error('starSquareRole failed:', e)
    }
  }

  return {
    roles,
    activeRole,
    squareRoles,
    squareTotal,
    stats,
    loading,
    squareLoading,
    filter,
    squareFilter,
    loadRoles,
    ensureRolesLoaded,
    loadStats,
    loadActiveRole,
    ensureActiveRoleLoaded,
    ensureRoleEventListener,
    createRole,
    updateRole,
    deleteRole,
    activateRole,
    deactivateRole,
    publishRole,
    unpublishRole,
    cloneRole,
    loadSquareRoles,
    applySquareRole,
    starSquareRole,
  }
})

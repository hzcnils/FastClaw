import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { DynamicMenu, MenuItem, WspCategory } from '@/types'

/**
 * Compatibility store for screens that still manage external services.
 * Public releases no longer expose database-driven external page menu entries.
 */
export const useMenuStore = defineStore('menu', () => {
  const wspMenuItems = ref<DynamicMenu[]>([])
  const menuLocale = ref('zh-CN')
  const isLoaded = ref(true)
  const wspCategories = computed<WspCategory[]>(() => [])
  const wspMenuData = computed<MenuItem[]>(() => [])

  function syncLocale(lang?: string) {
    menuLocale.value = String(lang || '').startsWith('en') ? 'en' : 'zh-CN'
  }
  function getMenuById(_menuId: string): DynamicMenu | undefined { return undefined }
  async function loadMenus() { isLoaded.value = true }
  async function addMenu(_menu: DynamicMenu) { /* removed in public edition */ }
  async function updateMenu(_menuId: string, _updates: Partial<DynamicMenu>) { /* removed */ }
  async function removeMenu(_menuId: string) { /* removed */ }
  async function reorderMenu(_menuId: string, _newOrder: number) { /* removed */ }

  return {
    wspMenuItems,
    wspCategories,
    wspMenuData,
    isLoaded,
    menuLocale,
    syncLocale,
    getMenuById,
    loadMenus,
    addMenu,
    updateMenu,
    removeMenu,
    reorderMenu,
  }
})

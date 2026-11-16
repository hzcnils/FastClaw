import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

export interface PlanActivity {
  path: string
  conversationId?: string
  entryId?: string
  busy: boolean
  updatedAt: number
}

export const usePlanActivityStore = defineStore('planActivity', () => {
  const byPath = ref<Record<string, PlanActivity>>({})

  function update(path: string, patch: Omit<Partial<PlanActivity>, 'path'>) {
    byPath.value = {
      ...byPath.value,
      [path]: { path, busy: false, ...byPath.value[path], ...patch, updatedAt: Date.now() },
    }
  }
  function remove(path: string) {
    const next = { ...byPath.value }
    delete next[path]
    byPath.value = next
  }
  const busyPaths = computed(() => Object.values(byPath.value).filter(item => item.busy).map(item => item.path))
  const anyBusy = computed(() => busyPaths.value.length > 0)
  return { byPath, busyPaths, anyBusy, update, remove }
})

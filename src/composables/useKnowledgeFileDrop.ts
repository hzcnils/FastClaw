import { nextTick, onActivated, onDeactivated, onMounted, onBeforeUnmount, watch, type Ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import { useKnowledgeBaseStore } from '@/stores/index'
import { fileDropRegistry } from '@/utils/fileDropRegistry'
import type { FilesystemSelection } from '@/components/chat/FilesystemBrowserModal.vue'

export interface KnowledgeSelectorHandle { open: () => void; close: () => void }

export function useKnowledgeFileDrop(options: {
  selectedIds: Ref<string[]>
  selector: Ref<KnowledgeSelectorHandle | null>
  session: () => unknown
  enabled?: () => boolean
}) {
  const route = useRoute()
  const path = route.path
  const store = useKnowledgeBaseStore()
  const { t } = useI18n()
  let unregister: (() => void) | undefined
  let generation = 0
  watch(options.session, () => { generation++ }, { flush: 'sync' })
  function deactivate() {
    generation++
    unregister?.()
    unregister = undefined
    options.selector.value?.close()
  }
  function activate() {
    if (unregister) return
    unregister = fileDropRegistry.register({
      path,
      enabled: () => route.path === path && (options.enabled?.() ?? true),
      session: () => generation,
      async receive(paths, isCurrent) {
        try {
          const result = await invoke<{ selections: FilesystemSelection[]; errors: { path: string; message: string }[] }>('inspect_drop_paths', { paths })
          if (!isCurrent()) return
          if (result.errors.length) ElMessage.warning(t('fileDrop.failedPaths', { count: result.errors.length }) + ': ' + result.errors.slice(0, 3).map(e => e.path).join('、'))
          if (!result.selections.length) return
          // One complete batch -> exactly one create call inside the Store.
          const kb = store.importLocalKnowledgeBatch(result.selections)
          if (!kb) return
          if (!kb.enabled) {
            ElMessage.warning(t('fileDrop.disabled'))
            return
          }
          if (!options.selectedIds.value.includes(kb.id)) options.selectedIds.value = [...options.selectedIds.value, kb.id]
          await nextTick()
          if (isCurrent()) {
            options.selector.value?.open()
            ElMessage.success(t('fileDrop.added', { count: kb.sources.length }))
          }
          void store.validateLocalKbs()
        } catch (error) {
          if (isCurrent()) ElMessage.error(t('fileDrop.failed') + ': ' + String(error))
        }
      },
    })
  }
  function beforeClose(event: Event) {
    if ((event as CustomEvent).detail?.path === path) deactivate()
  }
  onMounted(() => { activate(); window.addEventListener('fastclaw:before-tab-close', beforeClose) })
  onActivated(activate)
  onDeactivated(deactivate)
  onBeforeUnmount(() => { deactivate(); window.removeEventListener('fastclaw:before-tab-close', beforeClose) })
}

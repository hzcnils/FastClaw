import { computed } from 'vue'
import { type ChatContext, type ChatContextStore, type createChatContextBinding } from './contextBinding'
import { saveDocumentEditState } from './documentEditRuntime'
import type { ChatTabState } from './tabState'

interface DocumentEditorSession {
  key: string
  state: ChatTabState['state']
  events: { save(content: string): void; close(): void; cancel(): void; 'version-change'(idx: number): void }
}

/** Document drafts belong to a Tab; editor callbacks belong to one visible version. */
export function createDocumentEditorBinding<S extends ChatContextStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  options: { title(content: string, context: ChatContext<S>): string; panelWidth(): number },
) {
  let sequence = 0
  const actions = computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    const { owner, store } = lease.context, state = owner.state
    const versions = state.docVersions.value
    const current = () => lease.isCurrent() && state.docVersions.value === versions
    const persist = () => saveDocumentEditState(owner, store.currentConversationId)
    const show = () => {
      if (!state.editorVisible.value) {
        state.editorVisible.value = true
        state.isHistoryCollapsed.value = true
        state.chatPanelPx.value = options.panelWidth()
      }
    }
    function select(idx: number) {
      if (!current() || !Number.isInteger(idx) || idx < 0 || !versions[idx]) return
      state.activeVersionIdx.value = idx
      state.editorContent.value = versions[idx].content
      show(); persist()
    }
    return {
      open(content: string) {
        if (!current()) return
        if (versions.length) { select(0); return }
        state.editorContent.value = content
        state.editorTitle.value = options.title(content, lease.context)
        state.docVersions.value = [{ label: 'V1', content, timestamp: new Date().toISOString() }]
        state.activeVersionIdx.value = 0
        show(); persist()
      },
      select,
    }
  })
  const cards = computed(() => {
    const lease = binding.captureUi(), select = actions.value?.select
    const result: Record<string, () => void> = {}
    if (!lease?.isCurrent() || !select) return result
    const state = lease.context.owner.state
    for (const [id, card] of Object.entries(state.docEditCardMap)) {
      const index = card.versionIdx
      result[id] = () => {
        if (lease.isCurrent() && state.docEditCardMap[id] === card && card.versionIdx === index) select(index)
      }
    }
    return result
  })
  const session = computed<DocumentEditorSession | null>(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    const { owner, store } = lease.context, state = owner.state
    if (!state.editorVisible.value) return null
    // Deliberately exclude content: typing must not remount the editor or reset selection.
    const versions = state.docVersions.value, index = state.activeVersionIdx.value
    const version = versions[index]
    const select = actions.value?.select
    const token: DocumentEditorSession = { key: `document:${++sequence}`, state, events: {} as DocumentEditorSession['events'] }
    const current = () => lease.isCurrent() && session.value === token
      && state.editorVisible.value && state.docVersions.value === versions
      && state.activeVersionIdx.value === index && versions[index] === version
    const close = () => {
      if (!current()) return
      state.editorVisible.value = false
      state.isHistoryCollapsed.value = false
      saveDocumentEditState(owner, store.currentConversationId)
    }
    token.events = {
      close, cancel: close,
      save: content => { if (current()) state.editorContent.value = content },
      'version-change': idx => { if (current()) select?.(idx) },
    }
    return token
  })
  const panel = computed(() => {
    const value = session.value
    if (!value) return null
    const state = value.state
    return { key: value.key, content: state.editorContent.value, title: state.editorTitle.value,
      versions: state.docVersions.value, activeVersionIdx: state.activeVersionIdx.value, events: value.events }
  })
  return { actions, panel, cards }
}

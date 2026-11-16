import { computed } from 'vue'
import type { ChatContext, ChatContextStore, createChatContextBinding } from './contextBinding'

/** Controlled Composer fields only; no persistence, transport or mode-transition effects. */
export function createComposerContextBindings<S extends ChatContextStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  inactiveContext: ChatContext<S>,
) {
  // KeepAlive may render once after deactivation. Retain readable fields, but never writable leases.
  const state = computed(() => (binding.active.value?.context ?? inactiveContext).owner.state)
  const selectors = computed(() => ({
    modelId: state.value.modelId.value,
    knowledgeId: state.value.knowledgeId.value,
    executionMode: state.value.fastPlanExecutionMode.value,
    taskPlan: state.value.dialogueCharacter.value.taskPlan,
  }))
  const hints = computed(() => ({
    quotedText: state.value.quotedText.value,
    selectedSkill: state.value.selectedSkill.value,
  }))
  const hintEvents = computed(() => {
    const lease = binding.captureUi()
    const target = lease?.context.owner.state
    return {
      clearQuote: lease?.guard(() => { target!.quotedText.value = '' }),
      clearSkill: lease?.guard(() => { target!.selectedSkill.value = null }),
    }
  })
  const selectorUpdates = computed(() => {
    const lease = binding.captureUi()
    const target = lease?.context.owner.state
    return {
      // ModelSelector can emit undefined when cleared; TabState uses an empty-string sentinel.
      'update:modelId': lease?.guard((value: string | undefined) => { target!.modelId.value = value ?? '' }),
      'update:knowledgeId': lease?.guard((value: string[]) => { target!.knowledgeId.value = value }),
      'update:executionMode': lease?.guard((value: 'auto' | 'manual' | 'step') => { target!.fastPlanExecutionMode.value = value }),
    }
  })
  return { selectors, hints, hintEvents, selectorUpdates }
}

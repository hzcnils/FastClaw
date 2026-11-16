import type { ComputedRef, InjectionKey } from 'vue'

/** Nested conversations share the outermost active loading indicator. */
export const conversationLoadingKey: InjectionKey<ComputedRef<boolean>> = Symbol('conversation-loading')

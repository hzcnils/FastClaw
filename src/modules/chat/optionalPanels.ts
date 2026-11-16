// Module code is cached by import(); each visible panel owns its own component instance.
// Failed loads remain retryable. No import starts until a panel is requested.
export const loadMessageEditor = () => import('@/components/chat/EditMessageDialog.vue').then(module => module.default)
export const loadDocumentEditor = () => import('@/components/editor/DocumentEditor.vue').then(module => module.default)

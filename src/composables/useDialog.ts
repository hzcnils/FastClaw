import { ElMessageBox } from 'element-plus'
import { useI18n } from 'vue-i18n'

/** 高于自定义 Modal（z-index: 10050）；EP 的 zIndex 选项会被 nextZIndex() 覆盖，需靠 modalClass + CSS */
const MESSAGE_BOX_MODAL_CLASS = 'fc-msgbox-above-modal'

export function useDialog() {
  const { t } = useI18n()
  return {
    confirm: async (message: string, _options?: Record<string, unknown>): Promise<boolean> => {
      try {
        const opts = { ...(_options || {}) } as Record<string, unknown>
        const extraModalClass = typeof opts.modalClass === 'string' ? opts.modalClass : ''
        delete opts.modalClass
        await ElMessageBox.confirm(message, t('common.tip'), {
          type: 'warning',
          confirmButtonText: t('common.confirm'),
          cancelButtonText: t('common.cancel'),
          customClass: 'custom-confirm-dialog',
          modalClass: [MESSAGE_BOX_MODAL_CLASS, extraModalClass].filter(Boolean).join(' '),
          ...opts,
        })
        return true
      } catch {
        return false
      }
    },
    alert: (message: string, _options?: Record<string, unknown>): Promise<void> =>
      ElMessageBox.alert(message, t('common.tip'), {
        modalClass: MESSAGE_BOX_MODAL_CLASS,
        ...(_options || {}),
      }).then(() => undefined).catch(() => undefined),
  }
}

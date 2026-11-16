<script setup lang="ts">
// FastClaw · 关于我们弹窗
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import logoSvg from '@/assets/logo.svg?raw'
import { displayCompanyName } from '@/api/branding'

const { t } = useI18n()

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()
const visible = computed(() => props.modelValue)
function close() { emit('update:modelValue', false) }

// ── AboutusView 逻辑 ──

const modalBgStyle = computed(() => ({
  background: 'var(--theme-dailog-color)',
  border: '1px solid var(--border-color)',
  borderRadius: '18px',
  boxShadow: '0 24px 70px rgba(15, 23, 42, 0.24)',
}))
const modalTextStyle = computed(() => ({ color: 'var(--text-primary)' }))
const modalBorderStyle = computed(() => ({ borderBottomColor: 'var(--border-color)' }))
const appInfo = ref({ name: 'FastClaw', description: t('about.description') })
const showModal = ref(false)
const modalTitle = ref('')
const modalContent = ref('')
const linkContents = ref<Record<string, string>>({ feedback: '', license: '', author: '', changelog: '' })
const currentEditKey = ref('')
const editContent = ref('')
const isEditing = ref(false)
const defaultContents = computed<Record<string, string>>(() => ({
  license: 'Apache-2.0',
  author: displayCompanyName.value, changelog: t('about.noUpdate')
}))

const getKeyFromTitle = (title: string): string => {
  const map: Record<string, string> = { '意见反馈': 'feedback', '许可证': 'license', '作者': 'author', '更新日志': 'changelog' }
  return map[title] || ''
}
const startEdit = (title: string) => {
  const key = getKeyFromTitle(title)
  if (!key) return
  currentEditKey.value = key
  editContent.value = key === 'author'
    ? displayCompanyName.value
    : linkContents.value[key] || defaultContents.value[key] || ''
  isEditing.value = true
}
const saveEdit = async () => {
  if (!currentEditKey.value || !editContent.value.trim()) return
  try {
    await invoke('update_setting_with_category', { key: currentEditKey.value, value: editContent.value, category: 'about' })
    linkContents.value[currentEditKey.value] = editContent.value; modalContent.value = editContent.value; isEditing.value = false; currentEditKey.value = ''
  } catch (e) { console.error('Failed to save:', e) }
}
const cancelEdit = () => { isEditing.value = false; currentEditKey.value = ''; editContent.value = '' }
const loadAboutSettings = async () => {
  try {
    const rows: any[] = await invoke('get_settings', { category: 'about' })
    for (const r of rows) { linkContents.value[r.key] = r.value }
  } catch (e) { console.error('Failed to load about settings:', e) }
}
const initAboutSettings = async () => {
  try {
    const rows: any[] = await invoke('get_settings', { category: 'about' })
    if (rows.length === 0) {
      for (const [k, v] of Object.entries({ license: defaultContents.value.license }))
        await invoke('update_setting_with_category', { key: k, value: v, category: 'about' })
    }
    await loadAboutSettings()
  } catch (e) { console.error('Failed to init about settings:', e) }
}
onMounted(() => {
  initAboutSettings()
})
</script>

<template>
  <Teleport to="body">
    <Transition name="about-fade">
      <div v-if="visible" class="about-overlay" @click.self="close">
        <div class="about-panel">
          <header class="about-panel__head">
            <span class="about-panel__title">{{ $t('about.title') }}</span>
            <button class="about-panel__close" type="button" :aria-label="$t('common.close')" @click="close">×</button>
          </header>
          <div class="about-panel__body">
            <div class="about-view-container">
              <div class="about-content">
                <div class="about-card">
                  <div class="card-body">
                    <div class="app-info-row">
                      <div class="app-icon" aria-hidden="true"><span class="app-icon-svg" v-html="logoSvg" /></div>
                      <div class="app-details">
                        <div class="app-title-row">
                          <div class="app-name">{{ appInfo.name }}</div>
                        </div>
                      </div>
                    </div>
                    <div class="app-desc">{{ $t('about.description') }}</div>
                  </div>
                </div>
                <div class="links-card">
                  <!-- <div class="link-row"><span class="link-label">{{ $t('about.license') }}</span><span class="link-label">License</span></div> -->
                  <div class="link-row"><span class="link-label">{{ $t('about.author') }}</span><span class="link-label">{{ displayCompanyName }}</span></div>
                </div>
                <div v-if="showModal" class="modal-overlay" @click.self="isEditing ? cancelEdit() : (showModal = false)" style="background: rgba(15, 23, 42, 0.36); backdrop-filter: blur(10px);">
                  <div class="modal-dialog" :style="modalBgStyle">
                    <div class="modal-header" :style="modalBorderStyle"><span class="modal-title" :style="modalTextStyle">{{ modalTitle === '更新日志' ? $t('about.changelog') : modalTitle === '意见反馈' ? $t('about.feedback') : modalTitle === '许可证' ? $t('about.license') : modalTitle === '作者' ? $t('about.author') : modalTitle }}</span><button class="modal-close" :style="modalTextStyle" @click="isEditing ? cancelEdit() : (showModal = false)"><svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none"><path d="M18 6L6 18M6 6l12 12"/></svg></button></div>
                    <div class="modal-body" :style="modalTextStyle">
                      <div v-if="isEditing" class="edit-mode"><textarea v-model="editContent" class="edit-textarea" rows="6" :style="modalTextStyle"></textarea><div class="edit-actions"><button class="btn-cancel" @click="cancelEdit" hidden>取消</button><button class="btn-save" @click="saveEdit" hidden>保存</button></div></div>
                      <div v-else class="view-mode"><p>{{ modalContent }}</p><button class="btn-edit" @click="startEdit(modalTitle)" hidden>编辑</button></div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style lang="scss" scoped>
.about-overlay { position: fixed; inset: 0; z-index: 10001; display: flex; align-items: center; justify-content: center; background: rgba(0,0,0,0.15); overflow: hidden; }
.about-panel { width: 720px; max-width: calc(100vw - 48px); min-height: 600px; display: flex; flex-direction: column; background: var(--theme-dailog-color); color: var(--text-primary, #1f2329); border: 1px solid var(--border-color, #e5e6eb); border-radius: 16px; box-shadow: 0 24px 70px rgba(15,23,42,0.28); overflow: hidden; }
.about-panel__head { flex: 0 0 auto; display: flex; align-items: center; justify-content: space-between; padding: 14px 18px; border-bottom: 1px solid var(--border-color, #e5e6eb); }
.about-panel__title { font-size: 16px; font-weight: 600; }
.about-panel__close { padding-bottom: 6px; width: 32px; height: 32px; border-radius: 8px; transition: all .2s; border: none; background: transparent; font-size: 22px; line-height: 1; cursor: pointer; }
.about-panel__close:hover { background: var(--dropdown-hover-color); }
.about-panel__body { flex: 1 1 auto; min-height: 0; overflow: hidden; margin: 20px;}
.about-fade-enter-active,.about-fade-leave-active { transition: opacity .2s ease; }
.about-fade-enter-from,.about-fade-leave-to { opacity: 0; }
/* ── 内嵌 AboutusView 样式 ── */
.about-view-container { display: flex; width: 100%; height: 100%; overflow: hidden; }
.about-content { flex: 1; overflow-y: auto; }
.about-card { border: 1px solid var(--border-color); border-radius: 12px; margin-bottom: 16px; }
.card-header { padding: 16px 20px; border-bottom: 1px solid var(--border-color); }
.card-header .card-title { font-size: var(--font-size-base); font-weight: 600; color: var(--text-primary); }
.card-body { padding: 20px; }
.app-info-row { display: flex; align-items: flex-start; gap: 16px; }
.app-icon {
  width: 64px; height: 64px; flex-shrink: 0;
  display: flex; align-items: center; justify-content: center;
  border-radius: 16px; background: var(--color-primary); color: #fff; overflow: hidden;
}
.app-icon .app-icon-svg,
.app-icon .app-icon-svg :deep(svg) { width: 100%; height: 100%; display: block; }
.app-icon .app-icon-svg :deep(.cls-1) { stroke: transparent; }
.app-details { flex: 1; }
.app-details .app-title-row { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; margin-bottom: 4px; }
.app-details .app-name { font-size: var(--font-size-xl); font-weight: 600; color: var(--text-primary); }
.app-desc { margin-top: 10px; font-size: var(--font-size-md); color: var(--text-secondary); line-height: 1.7; }
.animate-spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.setting-label { font-size: var(--font-size-md); color: var(--text-primary); }
.links-card { border: 1px solid var(--border-color); border-radius: 12px; overflow: hidden; }
.link-row { display: flex; justify-content: space-between; align-items: center; padding: 14px 20px; border-bottom: 1px solid var(--border-color); cursor: pointer; transition: background .2s; }
.link-row:last-child { border-bottom: none; }
.link-row:hover { background: var(--bg-hover); }
.link-label { font-size: var(--font-size-base); color: var(--text-primary); }
.link-email { font-size: var(--font-size-base); color: var(--color-primary); text-decoration: none; background: none; border: none; padding: 0; cursor: pointer; font-family: inherit; }
.link-email:hover { text-decoration: underline; }
.btn-link { display: flex; align-items: center; gap: 4px; background: none; border: none; font-size: var(--font-size-md); color: var(--text-secondary); cursor: pointer; }
.btn-link:hover { color: var(--color-primary); }
.mail-picker-overlay { position: fixed; inset: 0; z-index: 10002; display: flex; align-items: center; justify-content: center; background: rgba(15, 23, 42, 0.3); }
.mail-picker-dialog { position: relative; width: 340px; padding: 24px; border-radius: 18px; display: flex; flex-direction: column; gap: 10px; }
.mail-picker-close {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background .2s, color .2s;
}
.mail-picker-close:hover {
  background: var(--dropdown-hover-color);
  color: var(--text-primary);
}
.mail-picker-title { font-size: var(--font-size-lg); font-weight: 700; text-align: center; margin-bottom: 4px; }
.mail-picker-recipient { text-align: center; font-size: var(--font-size-base); font-weight: 600; color: var(--color-primary); margin-bottom: 4px; word-break: break-all; }
.mail-picker-divider { display: flex; align-items: center; gap: 8px; margin: 2px 0; font-size: var(--font-size-xs); color: var(--text-secondary); }
.mail-picker-divider::before, .mail-picker-divider::after { content: ''; flex: 1; height: 1px; background: var(--border-color); }
.mail-picker-item { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border: 1px solid var(--border-color); border-radius: 12px; background: transparent; color: var(--text-primary); cursor: pointer; text-align: left; transition: all .18s ease; }
.mail-picker-item:hover { background: var(--bg-hover); border-color: var(--color-primary); transform: translateY(-1px); box-shadow: 0 2px 8px rgba(0,0,0,0.08); }
.mail-picker-item-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.mail-picker-item-name { font-size: var(--font-size-base); font-weight: 600; }
.mail-picker-item-desc { font-size: var(--font-size-xs); color: var(--text-secondary); }
.mail-picker-icon { width: 36px; height: 36px; border-radius: 10px; display: flex; align-items: center; justify-content: center; font-size: 12px; font-weight: 700; color: #fff; flex-shrink: 0; }
.mail-picker-icon--126 { background: #1a9f4a; }
.mail-picker-icon--qq { background: #12b7f5; }
.mail-picker-icon--163 { background: #e33b3b; }
.mail-picker-mailto { display: block; width: 100%; text-align: center; margin-top: 4px; padding: 10px 12px; border: none; border-radius: 10px; background: transparent; font-size: var(--font-size-sm); font-family: inherit; color: var(--text-secondary); text-decoration: none; cursor: pointer; }
.mail-picker-mailto:hover { color: var(--color-primary); background: var(--bg-hover); }
.modal-overlay { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal-dialog { width: 400px; max-height: 80vh; display: flex; flex-direction: column; overflow: hidden; color: var(--text-primary); }
.modal-header { display: flex; justify-content: space-between; align-items: center; padding: 16px 20px; border-bottom: 1px solid var(--border-color); flex-shrink: 0; background: transparent; }
.modal-header .modal-title { font-size: var(--font-size-base); font-weight: 600; }
.modal-header .modal-close { background: transparent; border: none; cursor: pointer; color: var(--text-secondary); padding: 0; display: flex; }
.modal-header .modal-close:hover { color: var(--text-primary); }
.modal-body { padding: 20px; overflow-y: auto; color: var(--text-primary); background: transparent; }
.modal-body p { margin: 0; font-size: var(--font-size-base); color: var(--text-primary); line-height: 1.6; }
.modal-body .view-mode { position: relative; }
.modal-body .view-mode p { white-space: pre-wrap; }
.modal-body .edit-mode .edit-textarea { width: 100%; padding: 12px; border: 1px solid var(--border-color); border-radius: 6px; font-size: var(--font-size-base); color: var(--text-primary); background: var(--bg-hover); resize: vertical; font-family: inherit; line-height: 1.5; }
.modal-body .edit-mode .edit-textarea:focus { outline: none; border-color: var(--color-primary); }
.modal-body .edit-mode .edit-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }
.modal-body .edit-mode .edit-actions .btn-cancel { padding: 6px 16px; background: transparent; border: 1px solid var(--border-color); border-radius: 6px; font-size: var(--font-size-md); color: var(--text-primary); cursor: pointer; }
.modal-body .edit-mode .edit-actions .btn-cancel:hover { background: var(--bg-hover); }
.modal-body .edit-mode .edit-actions .btn-save { padding: 6px 16px; background: var(--color-primary); border: none; border-radius: 6px; font-size: var(--font-size-md); color: white; cursor: pointer; }
.modal-body .edit-mode .edit-actions .btn-save:hover { opacity: .9; }
</style>

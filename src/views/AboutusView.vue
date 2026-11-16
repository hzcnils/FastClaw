<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useThemeStore } from '@/stores/theme'
import { invoke } from '@tauri-apps/api/core'
import logoSvg from '@/assets/logo.svg?raw'
import { displayCompanyName } from '@/api/branding'

const { t } = useI18n()
const themeStore = useThemeStore()

const modalBgStyle = computed(() => ({
  background: 'var(--theme-dailog-color)',
  border: '1px solid var(--border-color)',
  borderRadius: '18px',
  boxShadow: '0 24px 70px rgba(15, 23, 42, 0.24)',
}))

const modalTextStyle = computed(() => ({
  color: 'var(--text-primary)',
}))

const modalBorderStyle = computed(() => ({
  borderBottomColor: 'var(--border-color)',
}))

const appInfo = ref({
  name: 'FastClaw',
  description: t('about.description')
})

const showModal = ref(false)
const modalTitle = ref('')
const modalContent = ref('')

const linkContents = ref<Record<string, string>>({
  feedback: '',
  license: '',
  author: '',
  changelog: ''
})

const currentEditKey = ref('')
const editContent = ref('')
const isEditing = ref(false)

const getKeyFromTitle = (title: string): string => {
  const map: Record<string, string> = {
    '意见反馈': 'feedback',
    '许可证': 'license',
    '作者': 'author',
    '更新日志': 'changelog'
  }
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
    await invoke('update_setting_with_category', {
      key: currentEditKey.value,
      value: editContent.value,
      category: 'about'
    })
    linkContents.value[currentEditKey.value] = editContent.value
    modalContent.value = editContent.value
    isEditing.value = false
    currentEditKey.value = ''
  } catch (e) {
    console.error('Failed to save:', e)
  }
}

const cancelEdit = () => {
  isEditing.value = false
  currentEditKey.value = ''
  editContent.value = ''
}

const defaultContents = computed<Record<string, string>>(() => ({
  license: 'Apache-2.0',
  author: displayCompanyName.value,
  changelog: t('about.noUpdate')
}))

const openModal = async (title: string, key: string) => {
  await loadAboutSettings()
  modalTitle.value = title
  currentEditKey.value = getKeyFromTitle(title)
  modalContent.value = key === 'author'
    ? displayCompanyName.value
    : defaultContents.value[key] || linkContents.value[key] || ''
  showModal.value = true
  isEditing.value = false
}

const loadAboutSettings = async () => {
  try {
    const rows: any[] = await invoke('get_settings', { category: 'about' })
    console.log('Loaded settings:', rows)
    for (const r of rows) {
      linkContents.value[r.key] = r.value
    }
    console.log('Link contents:', linkContents.value)
  } catch (e) {
    console.error('Failed to load about settings:', e)
  }
}

const initAboutSettings = async () => {
  try {
    const rows: any[] = await invoke('get_settings', { category: 'about' })
    if (rows.length === 0) {
      await invoke('update_setting_with_category', { key: 'license', value: defaultContents.value.license, category: 'about' })
    }
    await loadAboutSettings()
  } catch (e) {
    console.error('Failed to init about settings:', e)
  }
}

onMounted(() => {
  initAboutSettings()
})
</script>
<template>
  <div class="about-view-container">
    <div class="about-content">
      <div class="about-card">
        <div class="card-header">
          <span class="card-title">{{ $t('about.title') }}</span>
        </div>
        <div class="card-body">
          <div class="app-info-row">
            <div class="app-icon" aria-hidden="true">
              <span class="app-icon-svg" v-html="logoSvg" />
            </div>
            <div class="app-details">
              <div class="app-title-row">
                <div class="app-name">{{ appInfo.name }}</div>
              </div>
            </div>
          </div>
          <div class="app-desc">{{ appInfo.description }}</div>
        </div>
      </div>

      <div class="links-card">
        <!-- <div class="link-row">
          <span class="link-label">{{ $t('about.license') }}</span>
          <span class="link-label">License</span> 
               <button class="btn-link" @click="openModal('许可证', 'license')">
            {{ $t('about.view') }}
            <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M9 18l6-6-6-6"></path>
            </svg>
          </button> 
        </div> -->
        <div class="link-row">
          <span class="link-label">{{ $t('about.author') }}</span>
          <span class="link-label">{{ displayCompanyName }}</span>
       
          <!-- <button class="btn-link" @click="openModal('作者', 'author')">
            {{ $t('about.view') }}
            <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M9 18l6-6-6-6"></path>
            </svg>
          </button> -->
        </div>
      </div>

      <div v-if="showModal" class="modal-overlay" @click.self="isEditing ? cancelEdit() : (showModal = false)" style="background: rgba(15, 23, 42, 0.36); backdrop-filter: blur(10px);">
        <div class="modal-dialog" :style="modalBgStyle">
          <div class="modal-header" :style="modalBorderStyle">
            <span class="modal-title" :style="modalTextStyle">{{ modalTitle === '更新日志' ? $t('about.changelog') : modalTitle === '意见反馈' ? $t('about.feedback') : modalTitle === '许可证' ? $t('about.license') : modalTitle === '作者' ? $t('about.author') : modalTitle }}</span>
            <button class="modal-close" :style="modalTextStyle" @click="isEditing ? cancelEdit() : (showModal = false)">
              <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
                <path d="M18 6L6 18M6 6l12 12"></path>
              </svg>
            </button>
          </div>
          <div class="modal-body" :style="modalTextStyle">
            <div v-if="isEditing" class="edit-mode">
              <textarea v-model="editContent" class="edit-textarea" rows="6" :style="modalTextStyle"></textarea>
              <div class="edit-actions">
                <button class="btn-cancel" @click="cancelEdit" hidden>取消</button>
                <button class="btn-save" @click="saveEdit" hidden>保存</button>
              </div>
            </div>
            <div v-else class="view-mode">
              <p>{{ modalContent }}</p>
              <button class="btn-edit" @click="startEdit(modalTitle)" hidden>编辑</button>
            </div>
          </div>
        </div>
      </div>

      <!-- 更新历史弹窗 -->
    </div>
  </div>
</template>
<style lang="scss" scoped>
.about-view-container {
  display: flex;
  width: 100%;
  height: 100%;
  // background: var(--bg-primary);
  overflow: hidden;
}

.about-content {
  flex: 1;
  overflow-y: auto;
}

.about-card {
  // background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  margin-bottom: 16px;
}

.card-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);

  .card-title {
    font-size: var(--font-size-base);
    font-weight: 600;
    color: var(--text-primary);
  }
}

.card-body {
  padding: 20px;
}

.app-info-row {
  display: flex;
  align-items: flex-start;
  gap: 16px;
}

.app-icon {
  width: 64px;
  height: 64px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 16px;
  background: var(--color-primary);
  color: #fff;
  overflow: hidden;

  .app-icon-svg {
    width: 100%;
    height: 100%;
    display: block;
  }

  .app-icon-svg :deep(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }

  /* 外框描边由容器背景承担，避免与主题底叠成双边 */
  .app-icon-svg :deep(.cls-1) {
    stroke: transparent;
  }
}

.app-details {
  flex: 1;

  .app-title-row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    margin-bottom: 4px;
  }

  .app-name {
    font-size: var(--font-size-xl);
    font-weight: 600;
    color: var(--text-primary);
  }
}

.app-desc {
  margin-top: 10px;
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  line-height: 1.7;
}

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.links-card {
  // background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  overflow: hidden;
}

.link-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 20px;
  border-bottom: 1px solid var(--border-color);
  cursor: pointer;
  transition: background 0.2s;

  &:last-child {
    border-bottom: none;
  }

  &:hover {
    background: var(--bg-hover);
  }

  .link-label {
    font-size: var(--font-size-base);
    color: var(--text-primary);
  }

  .link-email {
    color: var(--color-primary);
    font-size: var(--font-size-base);
    text-decoration: none;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    font-family: inherit;

    &:hover {
      text-decoration: underline;
    }
  }
}

.link-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.mail-picker-overlay {
  position: fixed;
  inset: 0;
  z-index: 10050;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(15, 23, 42, 0.3);
}

.mail-picker-dialog {
  position: relative;
  width: 340px;
  padding: 24px;
  border-radius: 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

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

.mail-picker-title {
  font-size: var(--font-size-lg);
  font-weight: 700;
  text-align: center;
  margin-bottom: 4px;
}

.mail-picker-recipient {
  text-align: center;
  font-size: var(--font-size-base);
  font-weight: 600;
  color: var(--color-primary);
  margin-bottom: 4px;
  word-break: break-all;
}

.mail-picker-mailto {
  display: block;
  width: 100%;
  text-align: center;
  margin-top: 4px;
  padding: 10px 12px;
  border: none;
  border-radius: 10px;
  background: transparent;
  font-size: var(--font-size-sm);
  font-family: inherit;
  color: var(--text-secondary);
  text-decoration: none;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    color: var(--color-primary);
    background: var(--bg-hover);
  }
}

.mail-picker-item {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 12px 14px;
  border: 1px solid var(--border-color);
  border-radius: 12px;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--font-size-base);
  cursor: pointer;
  text-decoration: none;
  text-align: left;
  transition: all 0.18s ease;
  line-height: 1.4;

  &:hover {
    background: var(--bg-hover);
    border-color: var(--color-primary);
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  }
}

.mail-picker-item-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.mail-picker-item-name {
  font-size: var(--font-size-base);
  font-weight: 600;
}

.mail-picker-item-desc {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
}

.mail-picker-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  color: #fff;
  flex-shrink: 0;
}

.mail-picker-icon--126 {
  background: #1a9f4a;
}

.mail-picker-icon--qq {
  background: #12b7f5;
}

.mail-picker-icon--163 {
  background: #e33b3b;
}

.mail-picker-divider {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 2px 0;
  font-size: var(--font-size-xs);
  color: var(--text-secondary);

  &::before,
  &::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--border-color);
  }
}

.btn-link {
  display: flex;
  align-items: center;
  gap: 4px;
  background: none;
  border: none;
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  cursor: pointer;

  &:hover {
    color: var(--color-primary);
  }
}

.modal-overlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-dialog {
  width: 400px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  color: var(--text-primary);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
  background: transparent;

  .modal-title {
    font-size: var(--font-size-base);
    font-weight: 600;
  }

  .modal-close {
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--text-secondary);
    padding: 0;
    display: flex;

    &:hover {
      color: var(--text-primary);
    }
  }
}

.modal-body {
  padding: 20px;
  overflow-y: auto;
  color: var(--text-primary);
  background: transparent;

  p {
    margin: 0;
    font-size: var(--font-size-base);
    color: var(--text-primary);
    line-height: 1.6;
  }

  .view-mode {
    position: relative;

    p {
      margin: 0;
      font-size: var(--font-size-base);
      color: var(--text-primary);
      line-height: 1.6;
      white-space: pre-wrap;
    }
  }

  .edit-mode {
    .edit-textarea {
      width: 100%;
      padding: 12px;
      border: 1px solid var(--border-color);
      border-radius: 6px;
      font-size: var(--font-size-base);
      color: var(--text-primary);
      background: var(--bg-hover);
      resize: vertical;
      font-family: inherit;
      line-height: 1.5;

      &:focus {
        outline: none;
        border-color: var(--color-primary);
      }
    }

    .edit-actions {
      display: flex;
      justify-content: flex-end;
      gap: 8px;
      margin-top: 12px;

      .btn-cancel {
        padding: 6px 16px;
        background: transparent;
        border: 1px solid var(--border-color);
        border-radius: 6px;
        font-size: var(--font-size-md);
        color: var(--text-primary);
        cursor: pointer;

        &:hover {
          background: var(--bg-hover);
        }
      }

      .btn-save {
        padding: 6px 16px;
        background: var(--color-primary);
        border: none;
        border-radius: 6px;
        font-size: var(--font-size-md);
        color: white;
        cursor: pointer;

        &:hover {
          opacity: 0.9;
        }
      }
    }
  }
}
</style>

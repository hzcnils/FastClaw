<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  OPACITY_CONTROLS,
  useThemeStore,
  type OpacityKey,
  type SceneThemeId,
  type SimpleThemeId,
  type ThemePreset,
} from '@/stores/theme'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (event: 'update:modelValue', value: boolean): void
}>()

const { t } = useI18n()
const themeStore = useThemeStore()
const activeTab = ref<'simple' | 'scene'>('simple')
const closeMode = ref<'commit' | 'revert' | null>(null)

const visible = computed(() => props.modelValue)
const selectedThemeId = computed(() => themeStore.draft.themeId)
const selectedSceneBaseId = computed(() => themeStore.draft.customBaseThemeId)

const customThemeBaseName = computed(() => {
  return t('theme.name_' + selectedSceneBaseId.value)
})

watch(
  () => props.modelValue,
  (isVisible, wasVisible) => {
    if (isVisible) {
      closeMode.value = null
      themeStore.beginDraft()
      activeTab.value = themeStore.draftCategory
      return
    }

    if (wasVisible && closeMode.value === null) {
      themeStore.cancelDraft()
    }
    closeMode.value = null
  }
)

function closeWithCancel() {
  closeMode.value = 'revert'
  themeStore.cancelDraft()
  emit('update:modelValue', false)
}

function confirmAndClose() {
  closeMode.value = 'commit'
  themeStore.confirmDraft()
  emit('update:modelValue', false)
}

function resetTheme() {
  themeStore.resetDraft()
  activeTab.value = 'simple'
}

function selectSimpleTheme(themeId: SimpleThemeId) {
  themeStore.selectTheme(themeId)
}

function selectSceneTheme(themeId: SceneThemeId) {
  themeStore.selectTheme(themeId)
}

function updateOpacity(key: OpacityKey, event: Event) {
  const input = event.target as HTMLInputElement
  themeStore.updateOpacity(key, Number(input.value))
}

function selectCustomTheme() {
  themeStore.selectCustomTheme()
}

function isThemeSelected(theme: ThemePreset) {
  return selectedThemeId.value === theme.id
}

function getPreviewStyle(theme: ThemePreset) {
  return {
    background: theme.preview.background,
    color: theme.preview.text,
  }
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && visible.value) {
    closeWithCancel()
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <Teleport to="body">
    <Transition name="theme-modal">
      <div v-if="visible" class="theme-modal-overlay" @click.self="closeWithCancel">
        <section class="theme-modal" role="dialog" aria-modal="true" aria-labelledby="theme-modal-title">
          <header class="theme-modal__header">
            <div>
              <p class="theme-modal__eyebrow">Appearance</p>
              <h2 id="theme-modal-title">{{ $t('theme.title') }}</h2>
            </div>
            <button class="theme-modal__close" type="button" :aria-label="$t('theme.closeTheme')" @click="closeWithCancel">
              <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </header>

          <div class="theme-tabs" role="tablist" :aria-label="$t('theme.category')">
            <button
              class="theme-tab"
              :class="{ active: activeTab === 'simple' }"
              type="button"
              role="tab"
              :aria-selected="activeTab === 'simple'"
              @click="activeTab = 'simple'"
            >
              {{ $t('theme.simpleThemes') }}
            </button>
            <button
              class="theme-tab"
              :class="{ active: activeTab === 'scene' }"
              type="button"
              role="tab"
              :aria-selected="activeTab === 'scene'"
              @click="activeTab = 'scene'"
            >
              {{ $t('theme.sceneThemes') }}
            </button>
          </div>

          <div class="theme-modal__body">
            <Transition name="theme-tab-pane" mode="out-in">
              <div v-if="activeTab === 'simple'" key="simple" class="theme-pane">
                <div class="theme-card-grid theme-card-grid--simple">
                  <button
                    v-for="theme in themeStore.simpleThemes"
                    :key="theme.id"
                    type="button"
                    class="theme-card"
                    :class="{ active: isThemeSelected(theme) }"
                    @click="selectSimpleTheme(theme.id as SimpleThemeId)"
                  >
                    <span v-if="isThemeSelected(theme)" class="theme-card__check">✓</span>
                    <span class="theme-card__preview" :style="getPreviewStyle(theme)">
                      <span class="preview-sidebar" :style="{ background: theme.preview.sidebar }"></span>
                      <span class="preview-main" :style="{ background: theme.preview.surface }">
                        <span class="preview-line preview-line--wide" :style="{ background: theme.preview.accent }"></span>
                        <span class="preview-line"></span>
                        <span class="preview-pill" :style="{ background: theme.preview.accent }"></span>
                      </span>
                    </span>
                    <span class="theme-card__name">{{ $t('theme.name_' + theme.id) }}</span>
                    <span class="theme-card__desc">{{ $t('theme.desc_' + theme.id) }}</span>
                  </button>
                </div>
              </div>

              <div v-else key="scene" class="theme-pane">
                <div class="theme-card-grid">
                  <button
                    v-for="theme in themeStore.sceneThemes"
                    :key="theme.id"
                    type="button"
                    class="theme-card"
                    :class="{ active: isThemeSelected(theme) }"
                    @click="selectSceneTheme(theme.id as SceneThemeId)"
                  >
                    <span v-if="isThemeSelected(theme)" class="theme-card__check">✓</span>
                    <span class="theme-card__preview" :style="getPreviewStyle(theme)">
                      <span class="preview-sidebar" :style="{ background: theme.preview.sidebar }"></span>
                      <span class="preview-main" :style="{ background: theme.preview.surface }">
                        <span class="preview-line preview-line--wide" :style="{ background: theme.preview.accent }"></span>
                        <span class="preview-line"></span>
                        <span class="preview-pill" :style="{ background: theme.preview.accent }"></span>
                      </span>
                    </span>
                    <span class="theme-card__name">{{ $t('theme.name_' + theme.id) }}</span>
                    <span class="theme-card__desc">{{ $t('theme.desc_' + theme.id) }}</span>
                  </button>
                </div>

                <section class="custom-theme">
                  <button
                    type="button"
                    class="custom-theme__preview"
                    :class="{ active: selectedThemeId === 'custom-scene' }"
                    @click="selectCustomTheme"
                  >
                    <span v-if="selectedThemeId === 'custom-scene'" class="theme-card__check">✓</span>
                    <span class="custom-theme__swatch" :style="{ background: themeStore.draftPreset.preview.background }">
                      <span :style="{ background: themeStore.draftPreset.preview.sidebar, opacity: themeStore.draft.opacity.mainMenu / 100 }"></span>
                      <span :style="{ background: themeStore.draftPreset.preview.surface, opacity: themeStore.draft.opacity.mainContent / 100 }"></span>
                      <span :style="{ background: themeStore.draftPreset.preview.accent }"></span>
                    </span>
                    <span>
                      <strong>{{ $t('theme.customTheme') }}</strong>
                      <small>{{ $t('theme.basedOnOpacity', { name: customThemeBaseName }) }}</small>
                    </span>
                  </button>

                  <div class="opacity-controls">
                    <label
                      v-for="item in OPACITY_CONTROLS"
                      :key="item.key"
                      class="opacity-control"
                    >
                      <span class="opacity-control__text">
                        <strong>{{ $t('theme.opacity_' + item.key) }}</strong>
                        <small>{{ $t('theme.opacity_' + item.key + '_desc') }}</small>
                      </span>
                      <span class="opacity-control__value">{{ themeStore.draft.opacity[item.key] }}%</span>
                      <input
                        type="range"
                        min="0"
                        max="100"
                        step="1"
                        :value="themeStore.draft.opacity[item.key]"
                        @input="updateOpacity(item.key, $event)"
                      />
                    </label>
                  </div>
                </section>
              </div>
            </Transition>
          </div>

          <footer class="theme-modal__footer">
            <button class="theme-action theme-action--secondary" type="button" @click="resetTheme">{{ $t('theme.reset') }}</button>
            <button class="theme-action theme-action--primary" type="button" @click="confirmAndClose">{{ $t('common.confirm') }}</button>
            <button class="theme-action theme-action--secondary" type="button" @click="closeWithCancel">{{ $t('common.cancel') }}</button>
          </footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style lang="scss" scoped>
.theme-modal-enter-active,
.theme-modal-leave-active {
  transition: opacity 0.2s ease;
}

.theme-modal-enter-active .theme-modal,
.theme-modal-leave-active .theme-modal {
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.theme-modal-enter-from,
.theme-modal-leave-to {
  opacity: 0;
}

.theme-modal-enter-from .theme-modal,
.theme-modal-leave-to .theme-modal {
  opacity: 0;
  transform: translateY(-8px) scale(0.98);
}

.theme-modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 10000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(15, 23, 42, 0.15);
  backdrop-filter: blur(0px);
}

.theme-modal {
  width: min(560px, 100%);
  max-height: min(86vh, 760px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  color: var(--text-primary);
  // background:
  //   linear-gradient(135deg, rgba(255, 255, 255, 0.06), transparent),
  //   var(--theme-dailog-color);
  background: var(--theme-dailog-color);
  border: 1px solid var(--border-color);
  border-radius: 18px;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);
}

.theme-modal__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 22px 14px;
  border-bottom: 1px solid var(--border-color);

  h2 {
    margin: 2px 0 0;
    font-size: var(--font-size-xl);
    font-weight: 700;
    letter-spacing: 0.2px;
  }
}

.theme-modal__eyebrow {
  margin: 0;
  color: var(--color-primary);
  font-size: var(--font-size-xs);
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.theme-modal__close {
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.18s ease;

  &:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
}

.theme-tabs {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
  padding: 14px 22px 0;
}

.theme-tab {
  position: relative;
  height: 38px;
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
  font-size: var(--font-size-base);
  font-weight: 600;
  transition: all 0.18s ease;

  &:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  &.active {
    color: var(--color-primary);
    background: var(--bg-active);
    border-color: var(--color-primary);
  }
}

.theme-modal__body {
  flex: 1;
  overflow-y: auto;
  padding: 16px 22px 18px;

  &::-webkit-scrollbar {
    width: 6px;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 999px;
  }
}

.theme-tab-pane-enter-active,
.theme-tab-pane-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.theme-tab-pane-enter-from,
.theme-tab-pane-leave-to {
  opacity: 0;
  transform: translateY(4px);
}

.theme-card-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.theme-card-grid--simple {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.theme-card,
.custom-theme__preview {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  padding: 10px;
  color: var(--text-primary);
  text-align: left;
  // background: var(--theme-card-bg, var(--bg-primary));
    background: var(--cancel-btn-color);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  cursor: pointer;
  transition: transform 0.18s ease, border-color 0.18s ease, box-shadow 0.18s ease, background 0.18s ease;

  &:hover {
    transform: translateY(-1px);
    border-color: var(--color-primary);
    box-shadow: 0 12px 28px rgba(15, 23, 42, 0.1);
  }

  &.active {
    border-color: var(--color-primary);
    box-shadow: 0 0 0 3px rgba(24, 144, 255, 0.18);
  }
}

.theme-card__preview {
  height: 76px;
  display: grid;
  grid-template-columns: 30% 1fr;
  gap: 7px;
  padding: 8px;
  overflow: hidden;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.14);
}

.preview-sidebar,
.preview-main {
  display: block;
  border-radius: 8px;
}

.preview-main {
  padding: 10px;
}

.preview-line {
  display: block;
  width: 60%;
  height: 6px;
  margin-bottom: 8px;
  background: currentColor;
  border-radius: 999px;
  opacity: 0.26;
}

.preview-line--wide {
  width: 78%;
  opacity: 0.9;
}

.preview-pill {
  display: block;
  width: 42%;
  height: 16px;
  margin-top: 11px;
  border-radius: 999px;
}

.theme-card__name {
  color: var(--text-primary);
  font-size: var(--font-size-md);
  font-weight: 700;
}

.theme-card__desc {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  line-height: 1.35;
}

.theme-card__check {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 1;
  width: 22px;
  height: 22px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  background: var(--color-primary);
  border-radius: 999px;
  font-size: var(--font-size-md);
  font-weight: 800;
  box-shadow: 0 6px 16px rgba(24, 144, 255, 0.28);
}

.custom-theme {
  margin-top: 16px;
  padding: 14px;
  // background: var(--theme-card-bg, var(--bg-primary));
  background: var(--cancel-btn-color);
  border: 1px solid var(--border-color);
  border-radius: 8px;
}

.custom-theme__preview {
  width: 100%;
  display: grid;
  grid-template-columns: 72px 1fr;
  align-items: center;
  gap: 12px;
  margin-bottom: 14px;

  strong,
  small {
    display: block;
  }

  strong {
    margin-bottom: 4px;
    font-size: var(--font-size-base);
  }

  small {
    color: var(--text-secondary);
    font-size: 12px;
  }
}

.custom-theme__swatch {
  width: 72px;
  height: 52px;
  display: grid;
  grid-template-columns: 24px 1fr;
  grid-template-rows: 1fr 12px;
  gap: 5px;
  padding: 6px;
  overflow: hidden;
  border-radius: 12px;

  span {
    border-radius: 7px;
  }

  span:first-child {
    grid-row: span 2;
  }

  span:last-child {
    grid-column: 2;
  }
}

.opacity-controls {
  display: grid;
  gap: 14px;
}

.opacity-control {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 8px 12px;
  align-items: center;
}

.opacity-control__text {
  strong,
  small {
    display: block;
  }

  strong {
    color: var(--text-primary);
    font-size: var(--font-size-md);
    font-weight: 650;
  }

  small {
    margin-top: 2px;
    color: var(--text-secondary);
    font-size: var(--font-size-xs);
  }
}

.opacity-control__value {
  color: var(--color-primary);
  font-size: 12px;
  font-weight: 700;
}

input[type='range'] {
  grid-column: 1 / -1;
  width: 100%;
  height: 4px;
  appearance: none;
  background: linear-gradient(90deg, var(--color-primary), var(--bg-hover));
  border-radius: 999px;
  outline: none;
  cursor: pointer;

  &::-webkit-slider-thumb {
    width: 16px;
    height: 16px;
    appearance: none;
    background: var(--color-primary);
    border: 3px solid var(--theme-dailog-color);
    border-radius: 50%;
    box-shadow: 0 3px 10px rgba(15, 23, 42, 0.2);
  }
}

.theme-modal__footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 22px 20px;
  border-top: 1px solid var(--border-color);
}

@media (max-width: 640px) {
  .theme-modal-overlay {
    padding: 12px;
  }

  .theme-modal {
    max-height: 92vh;
    border-radius: 8px;
  }

  .theme-card-grid,
  .theme-card-grid--simple {
    grid-template-columns: 1fr;
  }

  .theme-modal__footer {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
  }

  .theme-action {
    min-width: 0;
    padding: 0 10px;
  }
}
</style>

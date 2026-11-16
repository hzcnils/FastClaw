<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import SvgIcon from '@/components/svgIcon/index.vue'

const { t } = useI18n()

const PRESET_CHIPS = ['视觉', '联网', '推理', '工具', '重排', '嵌入']
const EXCLUSIVE_TAGS = ['重排', '嵌入']

const props = defineProps<{
  visible: boolean
  modelValue: string[]
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'update:modelValue', value: string[]): void
}>()

const customInput = ref('')
const selectedPresets = ref<string[]>([])

watch(() => props.visible, (val) => {
  if (val) {
    selectedPresets.value = [...props.modelValue]
    customInput.value = ''
  }
})

function close() {
  emit('update:visible', false)
}

function toggleTagSelectPreset(tag: string) {
  const arr = selectedPresets.value
  const idx = arr.indexOf(tag)
  if (idx >= 0) {
    arr.splice(idx, 1)
  } else {
    if (EXCLUSIVE_TAGS.includes(tag)) {
      // 选择互斥标签：清除所有普通固定标签 + 其他互斥标签，仅保留当前选中的
      selectedPresets.value = arr.filter(t => !PRESET_CHIPS.includes(t))
      if (!selectedPresets.value.includes(tag)) {
        selectedPresets.value.push(tag)
      }
    } else {
      // 选择普通标签：清除所有互斥标签
      selectedPresets.value = arr.filter(t => !EXCLUSIVE_TAGS.includes(t))
      if (!selectedPresets.value.includes(tag)) {
        selectedPresets.value.push(tag)
      }
    }
  }
}

function addTagSelectCustom() {
  const val = customInput.value.trim()
  if (val && val.length <= 20 && !selectedPresets.value.includes(val)) {
    selectedPresets.value.push(val)
  }
  customInput.value = ''
}

function removeTagSelectItem(index: number) {
  selectedPresets.value.splice(index, 1)
}

function confirmTagSelect() {
  const selected = [...new Set(selectedPresets.value)]
  emit('update:modelValue', selected)
  close()
}
</script>

<template>
  <div v-if="visible"
    style="position:fixed;inset:0;z-index:30000;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,0.5)">
    <div
      style="background:var(--theme-dailog-color);padding:20px;border-radius:12px;width:510px;max-height:70vh;overflow-y:auto;box-shadow:0 24px 70px rgba(0,0,0,0.3)">
      <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px">
        <h3 style="margin:0;font-size:17px;font-weight:600;">{{ $t('modelManage.selectTag') }}</h3>
        <button class="modal-close" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>

      <!-- 固定标签选择区 -->
      <div style="margin-bottom:16px">
        <div style="font-size:13px;color:var(--text-secondary);margin-bottom:8px">{{ $t('modelManage.presetTagHint') }}</div>
        <div class="preset-tags">
          <span v-for="chip in PRESET_CHIPS" :key="chip" class="preset-tag" :class="{
            active: selectedPresets.includes(chip),
            'exclusive-tag': EXCLUSIVE_TAGS.includes(chip)
          }" @click="toggleTagSelectPreset(chip)">
            <el-icon :size="14"><SvgIcon :iconName="chip" /></el-icon>
            {{ chip }}
          </span>
        </div>
      </div>

      <!-- 自定义标签输入 -->
      <div style="margin-bottom:16px">
        <div style="font-size:13px;color:var(--text-secondary);margin-bottom:8px">{{ $t('modelManage.customTag') }}</div>
        <div style="display:flex;gap:8px">
          <el-input v-model="customInput" size="small" class="tag-custom-input"
            :placeholder="$t('modelManage.customTagPlaceholder')" maxlength="20"
            style="flex:1; --el-input-bg-color: transparent; --el-input-border-color: var(--border-color);"
            :input-style="{ background: 'transparent' }"
            @keyup.enter="addTagSelectCustom" />
          <el-button class="addbtn" @click="addTagSelectCustom">{{ $t('modelManage.addTag') }}</el-button>
        </div>
      </div>

      <!-- 已选标签预览 -->
      <div style="margin-bottom:16px">
        <div style="font-size:13px;color:var(--text-secondary);margin-bottom:8px">
          {{ $t('modelManage.selectedTags', { n: selectedPresets.length }) }}
        </div>
        <div class="tag-input-area">
          <el-tag v-for="(tag, i) in selectedPresets" :key="i" closable size="small"
            @close="removeTagSelectItem(i)">{{ tag }}</el-tag>
          <span v-if="selectedPresets.length === 0" style="font-size:12px;color:var(--text-secondary)">
            {{ $t('modelManage.noTagSelected') }}
          </span>
        </div>
      </div>

      <!-- 底部按钮 -->
      <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:16px">
        <button @click="close" class="button-cancel">{{ $t('modelManage.cancel') }}</button>
        <button @click="confirmTagSelect" class="button-confirm">{{ $t('modelManage.confirm') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.preset-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.preset-tag {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 4px 12px;
  border: 1px solid var(--border-color);
  border-radius: 16px;
  cursor: pointer;
  font-size: 13px;
  color: var(--text-secondary);
  transition: all 0.2s;
  user-select: none;
}

.preset-tag:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.preset-tag.active {
  background: var(--color-primary);
  color: #fff;
  border-color: var(--color-primary);
}

.preset-tag.exclusive-tag {
  border-style: dashed;
}

.tag-input-area {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.modal-close {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-close:hover {
  color: var(--text-primary);
}

.tag-custom-input {
  --el-input-bg-color: transparent;
  --el-fill-color-blank: transparent;
}

:global(.tag-custom-input .el-input__wrapper),
:global(.tag-custom-input .el-input__wrapper.is-focus),
:global(.tag-custom-input .el-input__wrapper:hover) {
  background-color: transparent !important;
  background: transparent !important;
  box-shadow: none !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px;
}

:global(.tag-custom-input .el-input__inner),
:global(.tag-custom-input .el-input__inner:focus),
:global(.tag-custom-input .el-input__inner:hover) {
  background-color: transparent !important;
  background: transparent !important;
  color: var(--text-primary) !important;
}

:global(.tag-custom-input .el-input__inner::placeholder) {
  color: color-mix(in srgb, var(--text-primary) 55%, transparent) !important;
  opacity: 1;
}
</style>

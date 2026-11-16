<template>
  <el-tooltip
    v-if="isDisabled && !collapsed"
    :content="disabledTip"
    placement="right"
    effect="dark"
    :show-arrow="true"
    popper-class="menu-disabled-tip"
  >
    <div
      class="menu-item"
      :class="{ active, collapsed, highlight, disabled: isDisabled }"
      :title="collapsed ? displayLabel : undefined"
    >
      <span v-if="collapsed" class="item-abbr">
        <span v-if="isImage" class="item-icon-box">
          <img :src="resolvedIconSrc" class="item-icon-img" />
        </span>
        <span v-else-if="asSvgIcon && item.icon" class="item-icon-box">
          <SvgIcon :iconName="item.icon" />
        </span>
        <span v-else>{{ item.icon || item.label.slice(0, 1) }}</span>
      </span>
      <span v-else class="item-label">
        <span v-if="isImage" class="item-icon-box">
          <img :src="resolvedIconSrc" class="item-icon-img" />
        </span>
        <span v-else-if="asSvgIcon && item.icon" class="item-icon-box">
          <SvgIcon :iconName="item.icon" />
        </span>
        <span v-else-if="item.icon" class="item-icon">{{ item.icon }}</span>
        {{ displayLabel }}
      </span>
    </div>
  </el-tooltip>
  <div
    v-else
    class="menu-item"
    :class="{ active, collapsed, highlight, disabled: isDisabled }"
    @click="isDisabled ? undefined : emit('click')"
    :title="collapsed ? displayLabel : undefined"
  >
    <!-- 折叠状态：仅显示缩写/图标 -->
    <span v-if="collapsed" class="item-abbr">
      <span v-if="isImage" class="item-icon-box">
        <img :src="resolvedIconSrc" class="item-icon-img" />
      </span>
      <span v-else-if="asSvgIcon && item.icon" class="item-icon-box">
        <SvgIcon :iconName="item.icon" />
      </span>
      <span v-else>{{ item.icon || item.label.slice(0, 1) }}</span>
    </span>

    <!-- 展开状态：图标 + 文字 -->
    <span v-else class="item-label">
      <span v-if="isImage" class="item-icon-box">
        <img :src="resolvedIconSrc" class="item-icon-img" />
      </span>
      <span v-else-if="asSvgIcon && item.icon" class="item-icon-box">
        <SvgIcon :iconName="item.icon" />
      </span>
      <span v-else-if="item.icon" class="item-icon">{{ item.icon }}</span>
      {{ displayLabel }}
    </span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { MenuItem } from '@/types'
import SvgIcon from '@/components/svgIcon/index.vue'

const { t } = useI18n()

const props = defineProps<{
  item: MenuItem
  active: boolean
  highlight: boolean
  collapsed: boolean
  /** 图标解析器，用于WSP菜单的主题图标切换 */
  resolveIcon?: (icon: string) => string
  /** 是否直接使用原始 label（不经过 $t 翻译） */
  useRawLabel?: boolean
  /** 是否将 icon===label 时渲染为 SvgIcon */
  asSvgIcon?: boolean
  disabled?: boolean
  disabledReason?: string
}>()

const emit = defineEmits<{ click: [] }>()

const isDisabled = computed(() => props.disabled || props.item.disabled)
const disabledTip = computed(() => t(props.disabledReason || props.item.disabledReason || 'common.featureUnderDevelopment'))

const IMG_RE = /\.(png|jpe?g|gif|svg)(\?.*)?$/i

const isImage = computed(() => {
  const icon = props.item.icon
  return icon ? IMG_RE.test(icon) : false
})

const resolvedIconSrc = computed(() => {
  const icon = props.item.icon || ''
  if (isImage.value && props.resolveIcon) {
    return props.resolveIcon(icon)
  }
  return icon
})

const displayLabel = computed(() => {
  if (props.useRawLabel && props.item.rawLabel) {
    return props.item.label
  }
  return t(props.item.label)
})
</script>

<style lang="scss" scoped>
.menu-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  cursor: pointer;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  border-radius: 8px;
  margin: 2px 0;
  transition: all 0.2s ease;
  height: 40px;
  position: relative;
  z-index: 1;

  &:hover {
    background: var(--bg-hover);
  }

  &.active {
    background: var(--bg-active);
    color: var(--color-primary);
    font-weight: 600;
  }

  &.highlight {
    ////二级菜单背景颜色 筛选状态
    // background: rgba(255, 193, 7, 0.15);
    background:var(--treemenu-searching-color);
  //  box-shadow: inset 3px 0 0 #ff9800;
  }

  &.disabled {
    opacity: 0.45;
    cursor: not-allowed;

    &:hover {
      background: transparent !important;
    }
  }

  &.collapsed {
    width: 40px;
    height: 40px;
    padding: 0;
    justify-content: center;
    border-radius: 8px;
    margin: 2px 0;
    background: transparent;

    &:hover {
      background: var(--bg-hover);
    }

    &.active {
      background: var(--bg-active);
    }
  }

  .item-label {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  //一级菜单图标大小
  .item-icon {
    font-size: 18px;
  
  }

  .item-icon-box {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    overflow: hidden;
    border-radius: 4px;
     //一级菜单图标大小
    width: 18px;
    height: 18px;
  }

  .item-icon-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }

  .item-abbr {
    font-size: var(--font-size-base);
    font-weight: 500;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;

    .item-icon-box {
      width: 18px;
      height: 18px;
    }
  }
}
</style>

<style lang="scss">
/* 与 chat-history-dark-tooltip 同思路：实心底 + 箭头在气泡后方，避免菱形叠色/接缝 */
.menu-disabled-tip.el-popper,
.menu-disabled-tip.el-popper.is-dark {
  --menu-disabled-tip-bg: #303133;
  background: var(--menu-disabled-tip-bg) !important;
  background-color: var(--menu-disabled-tip-bg) !important;
  color: #fff !important;
  --el-text-color-primary: #fff !important;
  --el-text-color-regular: #fff !important;
  border: none !important;
  overflow: visible !important;
  filter: none !important;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
  box-shadow: none !important;
}

.menu-disabled-tip.el-popper .el-tooltip__content {
  color: inherit !important;
  background: transparent !important;
}

.menu-disabled-tip.el-popper > .el-popper__arrow {
  display: block !important;
  /* 必须在气泡后方：否则菱形压在边上会叠色、出现分割线 */
  z-index: -1 !important;
  width: 10px !important;
  height: 10px !important;
}

.menu-disabled-tip.el-popper > .el-popper__arrow::before {
  z-index: -1 !important;
  background: var(--menu-disabled-tip-bg) !important;
  background-color: var(--menu-disabled-tip-bg) !important;
  border: none !important;
  box-shadow: none !important;
  /* 覆盖 EP .is-dark > .el-popper__arrow:before { right:0 } 造成的错位 */
  right: auto !important;
  left: 0 !important;
}

/* 情景主题（除雾灰）：白底黑字，箭头同色无描边 */
html.light[data-theme-id^='scene-']:not([data-theme-id='scene-graphite-cyan']) .menu-disabled-tip.el-popper,
html.light[data-theme-id^='scene-']:not([data-theme-id='scene-graphite-cyan']) .menu-disabled-tip.el-popper.is-dark,
html.light[data-theme-id='custom-scene'] .menu-disabled-tip.el-popper,
html.light[data-theme-id='custom-scene'] .menu-disabled-tip.el-popper.is-dark {
  --menu-disabled-tip-bg: #ffffff;
  background: #fff !important;
  background-color: #fff !important;
  color: #000 !important;
  --el-text-color-primary: #000 !important;
  --el-text-color-regular: #000 !important;
  border: 1px solid rgba(0, 0, 0, 0.08) !important;
  box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1) !important;
}

html.light[data-theme-id^='scene-']:not([data-theme-id='scene-graphite-cyan']) .menu-disabled-tip.el-popper .el-tooltip__content,
html.light[data-theme-id='custom-scene'] .menu-disabled-tip.el-popper .el-tooltip__content {
  color: #000 !important;
}

html.light[data-theme-id^='scene-']:not([data-theme-id='scene-graphite-cyan']) .menu-disabled-tip.el-popper > .el-popper__arrow::before,
html.light[data-theme-id='custom-scene'] .menu-disabled-tip.el-popper > .el-popper__arrow::before {
  background: #fff !important;
  background-color: #fff !important;
  border: none !important;
}
</style>

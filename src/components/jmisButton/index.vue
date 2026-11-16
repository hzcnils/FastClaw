<template>
  <div class="head-but-panel">
    <template v-for="(list, index) in listItem" :key="index">
      <!-- 普通按钮 -->
      <el-button v-if="list.type === 'button'" style="margin-bottom: 10px" :type="list.typeColor" :icon="list.icon"
        color="#626aef" :disabled="(selected.length === 0 && list.index === 0) || list.disabled"
        @click="buttonClick(list.name)">
        {{ list.name }}
      </el-button>

      <!-- 下拉菜单按钮 -->
      <el-dropdown :style="{
        'margin-left': index == listItem.length - 1 ? '13px' : '0px',
      }" v-if="list.type === 'down'" split-button :type="list.typeColor" @click="buttonClick(list.name)"
        trigger="click" style="margin-right: 10px; margin-bottom: 10px">
        <el-icon>
          <component :is="list.icon" />
        </el-icon>
        {{ list.name }}
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item v-for="(item, i) in list.searchItem" :key="i" @click="buttonClick(item.name)">
              <el-icon>
                <component :is="item.icon" />
              </el-icon>
              {{ item.name }}
            </el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </template>

    <!-- 默认插槽 -->
    <slot></slot>
  </div>
</template>

<script setup>
import {
  ElButton,
  ElDropdown,
  ElDropdownItem,
  ElDropdownMenu,
  ElInput,
  ElIcon,
  ElScrollbar,
  ElForm,
  ElFormItem,
} from "element-plus";

const props = defineProps({
  listItem: {
    type: Array,
    default: () => [],
  },
  apiObj: {
    type: String,
    default: "",
  },
  selected: {
    type: Array,
    default: () => [],
  },
});

const emit = defineEmits(["buttonClick"]);

const buttonClick = (name) => {
  emit("buttonClick", name);
};
</script>


<style lang="scss" scoped>
.head-but-panel {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  margin-bottom: -10px;
}

:deep(.el-button--primary) {
  --el-button-bg-color: #626aef;
  --el-button-text-color: var(--el-color-white);
  --el-button-border-color: #626aef;
  --el-button-hover-bg-color: rgb(145, 151, 244);
  --el-button-hover-text-color: var(--el-color-white);
  --el-button-hover-border-color: rgb(145, 151, 244);
  --el-button-active-bg-color: rgb(82, 89, 195);
  --el-button-active-border-color: rgb(82, 89, 195);
}
</style>
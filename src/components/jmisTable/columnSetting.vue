<template>
  <div v-if="usercolumn.length > 0" class="setting-column" v-loading="isSave">
    <div class="setting-column__header">
      <span class="setting-column__header-title">{{ $t('dashboard.colSettingsTitle') }}</span>
      <button class="setting-column__close-btn" type="button" @click="$emit('close')">
        <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    </div>
    <div class="setting-column__title">
      <span class="move_b"></span>
      <span class="show_b">{{ $t('dashboard.colShow') }}</span>
      <span class="name_b">{{ $t('dashboard.colName') }}</span>
      <span class="width_b">{{ $t('dashboard.colWidth') }}</span>
      <span class="sortable_b">{{ $t('dashboard.colSort') }}</span>
      <span class="priority_b">{{ $t('dashboard.colPriority') }}</span>
      <span class="fixed_b">{{ $t('dashboard.colFixed') }}</span>
    </div>
    <div class="setting-column__list" ref="listRef">
      <ul>
        <li v-for="item in usercolumn" :key="item.prop">
          <span class="move_b">
            <el-tag class="move" style="cursor: move;"><el-icon :size="14"><DCaret /></el-icon></el-tag>
          </span>
          <span class="show_b">
            <el-switch v-model="item.hide" :active-value="false" :inactive-value="true"></el-switch>
          </span>
          <span class="name_b" :title="item.prop" style="padding-right: 20px;">
            <el-input v-model="item.label"></el-input>
          </span>
          <span class="width_b">
            <el-input v-model="item.width" placeholder="auto"></el-input>
          </span>
          <span class="sortable_b">
            <el-switch v-model="item.sortable"></el-switch>
          </span>
          <span class="priority_b">
            <el-input v-model="item.priority" type="number" max="50" min="1"></el-input>
          </span>
          <span class="fixed_b">
            <el-switch v-model="item.fixed"></el-switch>
          </span>
        </li>
      </ul>
    </div>
    <div class="setting-column__bottom">
      <el-button type="primary" plain @click="$emit('tableWidthChange', '初始化')">{{ $t('dashboard.initColumnWidth') }}</el-button>
      <el-button type="primary" plain @click="$emit('tableWidthChange', '自适应')">{{ $t('dashboard.autoColumnWidth') }}</el-button>
      <el-button @click="backDefaul" :disabled="isSave">{{ $t('common.reset') }}</el-button>
      <!-- <el-button @click="save" type="primary">保存</el-button> -->
      <el-dropdown split-button type="primary" @click="saveAll('全局')" trigger="click" :teleported="false" style="margin-left: 12px;">
        {{ $t('dashboard.globalSave') }}
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item @click="localStorageFn">{{ $t('dashboard.localCache') }}</el-dropdown-item>
            <el-dropdown-item @click="userSettingFn('个人')">{{ $t('dashboard.personalSettings') }}</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>

  </div>
  <el-empty v-else :description="$t('dashboard.noColumnsConfigured')" :image-size="80"></el-empty>
</template>

<script setup>
import { ref, watch, onMounted, inject, getCurrentInstance } from 'vue'
import Sortable from 'sortablejs';
import { ElTag, ElEmpty, ElButton, ElSwitch, ElMessage, ElInput,
 ElDropdown, ElDropdownMenu, ElDropdownItem, ElIcon
 } from 'element-plus';
import { DCaret } from '@element-plus/icons-vue'
import { useRoute } from 'vue-router';

const $TOOL = inject('$TOOL');
const route = useRoute();

const { proxy } = getCurrentInstance()

// 父组件传递的props
const props = defineProps({
  column: {
    type: Array,
    default: () => []
  },
  tableColumnsByBase: {
    type: Array,
    default: () => []
  },
  tableName: {
    type: String,
    default: ""
  }
})

// 自定义事件
const emit = defineEmits(['userChange', 'priority', 'back', 'save', 'close'])

// 响应式数据 (data)
const isSave = ref(false)
const usercolumn = ref(JSON.parse(JSON.stringify(props.column || [])))
const orderByList = ref([])
const listRef = ref(null);

// 监听器 (watch)
// 深度监听usercolumn
watch(() => usercolumn.value, (newVal) => {
  emit('userChange', newVal)
}, { deep: true })

// 挂载完成 (mounted)
onMounted(() => {
  usercolumn.value.length > 0 && rowDrop()
})

// 所有方法 (methods)
const getkeyName = (label) => {
  let keyName = null
  if (label) {
    props.tableColumnsByBase.some(item => {
      if (item.column_comment === label) {
        keyName = item.column_name
        return true
      }
    })
  }
  return keyName
}

// 拖拽功能
const rowDrop = () => {
  const tbody = listRef.value.querySelector('ul');
  Sortable.create(tbody, {
    handle: ".move",
    animation: 300,
    ghostClass: "ghost",
    onEnd({ newIndex, oldIndex }) {
      const tableData = usercolumn.value
      const currRow = tableData.splice(oldIndex, 1)[0]
      tableData.splice(newIndex, 0, currRow)
    }
  })
}

// 恢复默认
const backDefaul = () => {
  emit('back', usercolumn.value)
}

// 保存全部
const saveAll = (type) => {
  const data = [];
  for (let i = 0; i < usercolumn.value.length; i++) {
    if (usercolumn.value[i].sortable) {
      data.push({
        showName: getkeyName(usercolumn.value[i].label),
        javaName: usercolumn.value[i].prop,
        fieldName: usercolumn.value[i].prop,
        orderByType: usercolumn.value[i].asc ?? 'asc',
        orderByPriority: usercolumn.value[i].priority,
      })
    }
  }
  orderByList.value = data;
  emit('save', usercolumn.value, type, orderByList.value)
}

// 数据处理
const processData = () => {
  let json = []
  usercolumn.value.forEach(item => {
    let i = true
    copyData.value.forEach((item2) => {
      if (item.label == item2.column) {
        json.push({
          ...item,
          asc: item2.order
        })
        i = false
      }
    })
    if (i) {
      json.push(item)
    }
  })
  let data = {
    moduleId: route.name,
    tableName: props.tableName,
    userName: 'default',
    userId: 'default-user',
    dataJson: json
  }
  return data
}

// 本地缓存
const localStorageFn = () => {
  let data = processData()
  $TOOL.data.set('SORT', data)
  ElMessage.success('保存成功!')
}

// 个人设置
const userSettingFn = (type) => {
  const data = [];
  for (let i = 0; i < usercolumn.value.length; i++) {
    if (usercolumn.value[i].sortable) {
      data.push({
        showName: getkeyName(usercolumn.value[i].label),
        javaName: usercolumn.value[i].prop,
        fieldName: usercolumn.value[i].prop,
        orderByType: usercolumn.value[i].asc ?? 'asc',
        orderByPriority: usercolumn.value[i].priority,
      })
    }
  }
  orderByList.value = data;
  emit('save', usercolumn.value, type, orderByList.value)
}
</script>

<style scoped>
.setting-column__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 20px;
  border-bottom: 1px solid #EBEEF5;
}

.setting-column__header-title {
  font-size: 16px;
  font-weight: 600;
  color: #303133;
}

.setting-column__close-btn {
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #909399;
  background: transparent;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.18s ease;
}

.setting-column__close-btn:hover {
  color: #303133;
  background: rgba(0, 0, 0, 0.06);
}

.setting-column__title {
  border-bottom: 1px solid #EBEEF5;
  padding-bottom: 15px;
}

.setting-column__title span {
  display: inline-block;
  font-weight: bold;
  color: #909399;
  font-size: 12px;
}

.setting-column__title span.move_b {
  width: 30px;
  margin-right: 15px;
}

.setting-column__title span.show_b {
  width: 60px;
}

.setting-column__title span.name_b {
  width: 160px;
}

.setting-column__title span.width_b {
  width: 60px;
  margin-right: 15px;
}

.setting-column__title span.sortable_b {
  width: 60px;
}

.setting-column__title span.priority_b {
  width: 60px;
  margin-right: 15px;
}

.setting-column__title span.fixed_b {
  width: 60px;
}

.setting-column__list {
  max-height: 312px;
  overflow: auto;
}

.setting-column__list li {
  list-style: none;
  margin: 10px 0;
}

.setting-column__list li>span {
  display: inline-block;
  font-size: 12px;
}

.setting-column__list li span.move_b {
  width: 30px;
  margin-right: 15px;
}

.setting-column__list li span.show_b {
  width: 60px;
}

.setting-column__list li span.name_b {
  width: 160px;
}

.setting-column__list li span.width_b {
  width: 60px;
  margin-right: 15px;
}

.setting-column__list li span.sortable_b {
  width: 60px;
}

.setting-column__list li span.priority_b {
  width: 60px;
  margin-right: 15px;
}

.setting-column__list li span.fixed_b {
  width: 60px;
}

.setting-column__list li.ghost {
  opacity: 0.3;
}

.setting-column__bottom {
  border-top: 1px solid #EBEEF5;
  padding-top: 15px;
  text-align: right;
}

.priority_title {
  width: 100%;
  padding: 8px 0;
  border-top: 1px solid #EBEEF5;
  border-bottom: 1px solid #EBEEF5;
  margin-top: 8px;
}

.priority_title span {
  display: inline-block;
}

.priority_title span.move_c {
  width: 10%;
}

.priority_title span.numerical_c {
  width: 10%;
}

.priority_title span.column_c {
  width: 50%;
}

.priority_title span.order_c {
  width: 20%;
}

.priority_table {
  max-height: 440px;
  overflow: auto;
  border-bottom: 1px solid #EBEEF5;
}

.priority_table li {
  list-style: none;
  margin: 10px 0;
}

.priority_table li>span {
  display: inline-block;
  pointer-events: none;
}

.priority_table li span.move_c {
  width: 10%;
}

.priority_table li span.numerical_c {
  width: 10%;
}

.priority_table li span.column_c {
  width: 50%;
}

.priority_table li span.order_c {
  width: 20%;
}
</style>

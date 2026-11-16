<template>
  <div class="personal-assistant-panel">
    <el-tabs v-model="activeTab" class="panel-tabs" @tab-change="onTabChange">
      <el-tab-pane :label="$t('assistant.myRoles')" name="myRoles">
        <RoleList />
      </el-tab-pane>
      <el-tab-pane :label="$t('assistant.roleSquare')" name="square">
        <RoleSquare />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import RoleList from './RoleList.vue'
import RoleSquare from './RoleSquare.vue'

const activeTab = ref('myRoles')

function onTabChange(tab: string) {
  activeTab.value = tab
}
</script>

<style scoped lang="scss">
.personal-assistant-panel {
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;

  .panel-tabs {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;

    :deep(.el-tabs__header) {
      flex-shrink: 0;
      margin: 0 0 14px 0;
    }

    :deep(.el-tabs__content) {
      flex: 1;
      overflow: hidden;

      .el-tab-pane {
        height: 100%;
        overflow: hidden;
      }
    }
  }
}
</style>

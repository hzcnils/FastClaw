<template>
  <nav class="tree-menu">
    <div class="menu-top-actions" :class="{ 'is-collapsed': collapsed }">
      <div v-if="!collapsed" class="search-box">
        <input
          v-model="searchQuery"
          class="search-input"
          :placeholder="$t('sidebar.searchMenu')"
          @keyup.esc="searchQuery = ''"
        />
        <SearchClearButton v-if="searchQuery" overlay @clear="searchQuery = ''" />
      </div>
      <button
        type="button"
        class="collapse-btn"
        :class="{ 'is-collapsed': collapsed }"
        :title="$t('sidebar.toggleMenu')"
        @click.stop.prevent="$emit('toggle-collapse')"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 18 16" aria-hidden="true">
          <path
            d="M121.007,185.666a.985.985,0,0,1-.968,1h-15.5a1,1,0,0,1,0-2h15.5a.985.985,0,0,1,.968,1ZM107.4,171.423v7.486a.5.5,0,0,1-.3.462.473.473,0,0,1-.528-.109l-3.282-3.389a1.023,1.023,0,0,1,0-1.414l3.283-3.39a.473.473,0,0,1,.527-.108.5.5,0,0,1,.3.462Zm13.607,7.243a.985.985,0,0,1-.968,1h-6.779a1,1,0,0,1,0-2h6.779A.985.985,0,0,1,121.007,178.666Zm0-7a.985.985,0,0,1-.968,1h-6.779a1,1,0,0,1,0-2h6.779A.985.985,0,0,1,121.007,171.666Z"
            transform="translate(-103.007 -170.667)" fill="currentColor" />
        </svg>
      </button>
    </div>

    <el-scrollbar class="menu-scrollbar">
      <div class="menu-list">
        <MenuItemComponent
          v-for="item in visiblePrimaryMenus"
          :key="item.id"
          :item="item"
          :collapsed="collapsed"
          :active="isActive(item.route || '')"
          :highlight="false"
          :as-svg-icon="true"
          @click="navigateTo(item.route || '')"
        />

        <div v-if="visiblePlatformMenus.length" class="menu-group">
          <el-popover
            v-if="collapsed"
            trigger="click"
            placement="right-start"
            :offset="4"
            :show-arrow="false"
            popper-class="tree-menu-popover"
          >
            <template #reference>
              <button class="group-header collapsed" :title="$t('sidebar.modelPlatform')">
                <span class="group-icon-box"><SvgIcon iconName="模型中台" /></span>
              </button>
            </template>
            <div class="popover-menu-list">
              <MenuItemComponent
                v-for="item in visiblePlatformMenus"
                :key="item.id"
                :item="item"
                :collapsed="false"
                :active="isActive(item.route || '')"
                :highlight="false"
                :as-svg-icon="true"
                @click="navigateTo(item.route || '')"
              />
            </div>
          </el-popover>
          <template v-else>
            <button class="group-header" @click="platformExpanded = !platformExpanded">
              <span class="group-icon-box"><SvgIcon iconName="模型中台" /></span>
              <span>{{ $t('sidebar.modelPlatform') }}</span>
              <span class="group-arrow">{{ platformExpanded ? '▾' : '▸' }}</span>
            </button>
            <div v-if="platformExpanded" class="group-children">
              <MenuItemComponent
                v-for="item in visiblePlatformMenus"
                :key="item.id"
                :item="item"
                :collapsed="false"
                :active="isActive(item.route || '')"
                :highlight="false"
                :as-svg-icon="true"
                @click="navigateTo(item.route || '')"
              />
            </div>
          </template>
        </div>

        <div v-if="!visiblePrimaryMenus.length && !visiblePlatformMenus.length" class="search-empty">
          {{ $t('sidebar.noSearchResults') }}
        </div>
      </div>
    </el-scrollbar>
  </nav>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import type { MenuItem } from '@/types'
import MenuItemComponent from '@/components/common/MenuItem.vue'
import SearchClearButton from '@/components/common/SearchClearButton.vue'
import SvgIcon from '@/components/svgIcon/index.vue'

defineProps<{ collapsed: boolean }>()
defineEmits<{ (event: 'toggle-collapse'): void }>()

const router = useRouter()
const route = useRoute()
const { t } = useI18n()
const searchQuery = ref('')
const platformExpanded = ref(true)

const primaryMenus: MenuItem[] = [
  { id: 'chat', label: 'sidebar.smartChat', icon: '智能对话', route: '/chat' },
  { id: 'knowledge', label: 'sidebar.knowledge', icon: '知识管理', route: '/knowledge' },
]

const platformMenus: MenuItem[] = [
  { id: 'models', label: 'sidebar.modelManage', icon: '模型管理', route: '/models' },
  { id: 'alltool', label: 'sidebar.allTools', icon: '全部工具', route: '/alltool' },
  { id: 'skills', label: 'sidebar.skillsManage', icon: 'Skills管理', route: '/skills' },
  { id: 'mcp', label: 'sidebar.mcpService', icon: 'MCP服务', route: '/mcp' },
  { id: 'wsp', label: 'sidebar.wspManage', icon: 'WSP管理', route: '/wsp-admin' },
  { id: 'dashboard', label: 'sidebar.callDashboard', icon: '调用看板', route: '/dashboard' },
]

const matches = (item: MenuItem, query: string) => t(item.label).toLowerCase().includes(query)
const visiblePrimaryMenus = computed(() => {
  const query = searchQuery.value.trim().toLowerCase()
  return query ? primaryMenus.filter(item => matches(item, query)) : primaryMenus
})
const visiblePlatformMenus = computed(() => {
  const query = searchQuery.value.trim().toLowerCase()
  if (!query || t('sidebar.modelPlatform').toLowerCase().includes(query)) return platformMenus
  return platformMenus.filter(item => matches(item, query))
})

function isActive(path: string): boolean {
  if (path === '/chat') return route.path === '/chat' || route.path.startsWith('/chat/')
  if (path === '/knowledge') {
    return route.path === '/knowledge' || route.path.startsWith('/knowledge-bases/')
  }
  return route.path === path || route.path.startsWith(`${path}/`)
}

function navigateTo(path: string): void {
  if (!path) return
  if (path === '/chat') {
    const instanceId = Date.now().toString(36) + Math.random().toString(36).slice(2, 6)
    void router.push({ path: `/chat/${instanceId}`, query: { title: 'sidebar.smartChat' } })
    return
  }
  void router.push(path)
}
</script>

<style scoped>
.tree-menu { display: flex; flex-direction: column; height: 100%; color: var(--text-primary); }
.menu-top-actions { display: flex; align-items: center; gap: 8px; padding: 8px; }
/* 折叠态：不用整条居中，改成与 .menu-list 同一几何（左右 8px + 40px 槽靠左），
   否则按钮会落在 69px 的正中(34.5px)，而下方图标中心在 28px，差 6.5px。
   40 − 33 = 7，左右各补 3.5px 让 33px 按钮与 40px 菜单项同轴。 */
.menu-top-actions.is-collapsed { justify-content: flex-start; }
.menu-top-actions.is-collapsed .collapse-btn { margin-left: 3.5px; }
.search-box { position: relative; flex: 1; }
.search-input { box-sizing: border-box; width: 100%; height: 32px; padding: 0 30px 0 10px; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-secondary); color: var(--text-primary); outline: none; }
.collapse-btn, .group-header { border: 0; background: transparent; color: var(--text-secondary); cursor: pointer; }
/* 与对话页 ChatHistorySidebar.vue 的 .btn-collapse 对齐：33×33、1px 描边、圆角 4px、轻阴影 */
.collapse-btn { width: 33px; height: 33px; flex-shrink: 0; border: 1px solid var(--border-color); border-radius: 4px; display: flex; align-items: center; justify-content: center; color: var(--text-primary); transition: all 0.2s; box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06); }
/* 折叠态把同一图标水平镜像，方向语义与展开/收起一致 */
.collapse-btn.is-collapsed svg { transform: scaleX(-1); }
/* 修正：--hover-bg 全仓未定义（通用变量是 --bg-hover），原来这句的 hover 背景一直是失效的 */
.collapse-btn:hover, .group-header:hover { background: var(--bg-hover); color: var(--text-primary); }
.menu-scrollbar { flex: 1; min-height: 0; }
.menu-list { display: flex; flex-direction: column; gap: 4px; padding: 4px 8px; }
.menu-group { display: flex; flex-direction: column; gap: 3px; }
.group-header { display: flex; align-items: center; width: 100%; height: 40px; gap: 8px; padding: 0 12px; border-radius: 8px; text-align: left; }
/* 与 MenuItem.vue 的 .item-icon-box 同规格：SvgIcon 自带 width="24"，靠这层 18×18 的 flex 容器缩到与一级菜单一致 */
.group-icon-box { display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; overflow: hidden; border-radius: 4px; width: 18px; height: 18px; }
/* 折叠态对齐 .menu-item.collapsed（40×40、圆角 8px、上下 2px 间距） */
.group-header.collapsed { width: 40px; height: 40px; margin: 2px 0; padding: 0; justify-content: center; }
.group-arrow { margin-left: auto; }
.group-children { display: flex; flex-direction: column; gap: 3px; padding-left: 12px; }
.popover-menu-list { min-width: 170px; }
.search-empty { padding: 24px 8px; color: var(--text-secondary); text-align: center; font-size: 13px; }
</style>

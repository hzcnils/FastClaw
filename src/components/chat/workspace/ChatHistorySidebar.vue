<script setup lang="ts">
import { computed, onBeforeUnmount, onDeactivated, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, Star, StarFilled, ChatDotSquare } from '@element-plus/icons-vue'
import type { DropdownInstance } from 'element-plus'
import ConversationStatusBadge from '@/components/chat/ConversationStatusBadge.vue'
import type { Conversation } from '@/types'
import type { FavoriteHistoryItem, HistoryActivitySummary } from '@/modules/chat/historySidebarPresentation'

const props = defineProps<{
  conversations: readonly Pick<Conversation, 'id' | 'title' | 'assistant_id' | 'extra_config'>[]
  currentConversationId: string | null
  collapsed: boolean
  active: boolean
  asideTab: 'history' | 'favorites'
  favItems: readonly FavoriteHistoryItem[]
  favLoading: boolean
  activities: HistoryActivitySummary
}>()
const emit = defineEmits<{
  newChat: []
  select: [conversationId: string]
  rename: [conversationId: string]
  delete: [conversationId: string]
  collapse: []
  tabChange: [tab: 'history' | 'favorites']
  favoriteOpen: [favoriteId: string, conversationId: string | null, messageId: string | null]
  favoriteRemove: [favoriteId: string]
}>()
const { t } = useI18n()
// Directory-only derivation: do not reparse potentially large extra_config on token/badge updates.
const historyRows = computed(() => props.conversations.map(conv => {
  let config: { mcp?: unknown } | null = null
  try { config = conv.extra_config ? JSON.parse(conv.extra_config) : null } catch { /* invalid legacy config */ }
  return { id: conv.id, title: conv.title, assistant_id: conv.assistant_id,
    mcp: config?.mcp === true }
}))
const hoveredConvId = ref<string | null>(null);
const openMenuConvId = ref<string | null>(null);
const historyMenuLabels = computed(() => ({ edit: t('common.edit'), delete: t('common.delete') }));
/** 历史/收藏标题：仅截断时显示完整标题提示 */
const convTitleTipId = ref<string | null>(null);
let convTitleTipTimer: ReturnType<typeof setTimeout> | null = null;

function clearConvTitleTipTimer() {
  if (convTitleTipTimer != null) {
    clearTimeout(convTitleTipTimer);
    convTitleTipTimer = null;
  }
}

function hideConvTitleTip() {
  clearConvTitleTipTimer();
  convTitleTipId.value = null;
}

function isTextTruncated(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  try {
    return el.scrollWidth > el.clientWidth + 1;
  } catch {
    return false;
  }
}

function onConvTitleEnter(id: string, event: MouseEvent) {
  hideConvTitleTip();
  if (!props.active || props.collapsed) return;
  if (!isTextTruncated(event.currentTarget)) return;
  convTitleTipTimer = setTimeout(() => {
    convTitleTipId.value = id;
  }, 400);
}

function onConvTitleLeave(id: string) {
  clearConvTitleTipTimer();
  if (convTitleTipId.value === id) convTitleTipId.value = null;
}

// ElDropdown exposes handleClose; "visible" is not a controlled prop.
const menuRefs = new Map<string, Pick<DropdownInstance, 'handleClose'>>()
function setMenuRef(id: string, instance: unknown) {
  const menu = instance as DropdownInstance | null
  if (menu?.handleClose) menuRefs.set(id, menu)
  else menuRefs.delete(id)
}
function onMenuVisibleChange(convId: string, visible: boolean) {
  if (!visible) {
    if (openMenuConvId.value === convId) openMenuConvId.value = null
    return
  }
  if (!props.active || props.collapsed) {
    menuRefs.get(convId)?.handleClose()
    return
  }
  if (openMenuConvId.value && openMenuConvId.value !== convId) {
    menuRefs.get(openMenuConvId.value)?.handleClose()
  }
  openMenuConvId.value = convId
}

function onMenuAction(action: 'rename' | 'delete', id: string) {
  menuRefs.get(id)?.handleClose()
  resetTransientState()
  if (action === 'rename') emit('rename', id)
  else emit('delete', id)
}
function resetTransientState() {
  hideConvTitleTip()
  hoveredConvId.value = null
  if (openMenuConvId.value) menuRefs.get(openMenuConvId.value)?.handleClose()
  openMenuConvId.value = null
}
watch(() => [props.active, props.collapsed, props.asideTab, props.currentConversationId], resetTransientState, { flush: 'post' })
onDeactivated(resetTransientState)
onBeforeUnmount(resetTransientState)

function favContentPreview(content: string, maxLen = 60): string {
  const cleaned = content.replace(/<[^>]+>/g, '').replace(/\n+/g, ' ')
  return cleaned.length > maxLen ? cleaned.slice(0, maxLen) + '…' : cleaned
}
</script>

<template>
    <aside class="conv-aside" :class="{ collapsed: collapsed }">
      <div class="conv-aside-head">
        <button class="btn-new-chat" @click="emit('newChat')">
          <el-icon>
            <Plus />
          </el-icon>
          <span>{{ t("chat.newChat") }}</span>
        </button>
        <el-tooltip :content="t('chat.collapseSidebarTitle')" placement="top" effect="dark" :show-after="300"
          :enterable="false" :hide-after="0" popper-class="chat-history-dark-tooltip">
          <button class="btn-collapse" @click="emit('collapse')">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 18 16">
              <path id="路径_5210" data-name="路径_5210"
                d="M121.007,185.666a.985.985,0,0,1-.968,1h-15.5a1,1,0,0,1,0-2h15.5a.985.985,0,0,1,.968,1ZM107.4,171.423v7.486a.5.5,0,0,1-.3.462.473.473,0,0,1-.528-.109l-3.282-3.389a1.023,1.023,0,0,1,0-1.414l3.283-3.39a.473.473,0,0,1,.527-.108.5.5,0,0,1,.3.462Zm13.607,7.243a.985.985,0,0,1-.968,1h-6.779a1,1,0,0,1,0-2h6.779A.985.985,0,0,1,121.007,178.666Zm0-7a.985.985,0,0,1-.968,1h-6.779a1,1,0,0,1,0-2h6.779A.985.985,0,0,1,121.007,171.666Z"
                transform="translate(-103.007 -170.667)" fill="currentColor" />
            </svg>
          </button>
        </el-tooltip>
      </div>
      <div class="conv-aside-title conv-aside-title-row">
        <span>{{ asideTab === 'history' ? t('chat.historyConversations') : t('chat.favoriteMessages') }}</span>
        <el-tooltip
          :content="asideTab === 'history' ? t('chat.favorite') : t('chat.historyConversations')"
          placement="top" effect="dark" :show-after="300" :enterable="false" :hide-after="0"
          popper-class="chat-history-dark-tooltip">
          <button class="aside-tab-btn"
            @click="emit('tabChange', asideTab === 'history' ? 'favorites' : 'history')">
            <el-icon>
              <Star v-if="asideTab === 'history'" />
              <ChatDotSquare v-else />
            </el-icon>
          </button>
        </el-tooltip>
      </div>
      <!-- 历史会话列表 -->
      <div v-if="asideTab === 'history'" class="conv-list">
        <div v-for="conv in historyRows" :key="conv.id" class="conv-item" :class="{
          active: currentConversationId === conv.id,
          hover: hoveredConvId === conv.id,
        }" @mousedown.left="hoveredConvId = null; hideConvTitleTip()" @click="emit('select', conv.id)"
          @mouseenter="hoveredConvId = conv.id" @mouseleave="hoveredConvId = null; hideConvTitleTip()">
          <span class="conv-title-group">
            <el-tooltip :content="conv.title || t('chat.newChat')" placement="top" effect="dark"
              :visible="convTitleTipId === conv.id" :enterable="false" :hide-after="0"
              :gpu-acceleration="false" transition="" popper-class="chat-history-dark-tooltip"
              @update:visible="(v: boolean) => { if (!v && convTitleTipId === conv.id) hideConvTitleTip() }">
              <span
                class="conv-title"
                @mouseenter="onConvTitleEnter(conv.id, $event)"
                @mouseleave="onConvTitleLeave(conv.id)"
              >{{ conv.title || t("chat.newChat") }}</span>
            </el-tooltip>
            <ConversationStatusBadge :streaming="activities[conv.id]?.streaming"
              :status="activities[conv.id]?.phase"
              :unread="activities[conv.id]?.unread ?? false" />
          </span>
          <span v-if="conv.assistant_id || conv.mcp" class="conv-tags">
            <el-tag v-if="conv.assistant_id" class="conv-agent-tag" size="small" type="success" effect="light" round>
              Agent
            </el-tag>
            <el-tag v-if="conv.mcp" class="conv-agent-tag conv-mcp-tag" size="small" type="warning"
              effect="light" round>
              {{ t('chat.mcpTool') }}
            </el-tag>
          </span>
          <el-dropdown trigger="click" :persistent="false" :ref="setMenuRef.bind(null, conv.id)"
            @visible-change="onMenuVisibleChange(conv.id, $event)" placement="bottom-end">
            <button class="conv-more" @click.stop>···</button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click.stop="onMenuAction('rename', conv.id)">{{
                  historyMenuLabels.edit
                }}</el-dropdown-item>
                <el-dropdown-item @click.stop="onMenuAction('delete', conv.id)">{{
                  historyMenuLabels.delete
                }}</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
        <div v-if="!historyRows.length" class="empty-tip">
          {{ t("chat.noHistoryConversations") }}
        </div>
      </div>
      <!-- 收藏列表 -->
      <div v-else class="conv-list fav-list">
        <div v-if="favLoading" class="fav-tip">{{ t('common.loading') }}</div>
        <div v-else-if="!favItems.length" class="fav-tip">暂无收藏</div>
        <div v-for="item in favItems" :key="item.id" class="conv-item fav-item"
          :class="{ 'is-deleted': item.conversation_deleted }" @click="emit('favoriteOpen', item.id, item.conversation_id, item.message_id)">
          <el-tooltip content="取消收藏" placement="top" effect="dark" :show-after="300"
            :enterable="false" :hide-after="0" popper-class="chat-history-dark-tooltip">
            <button class="fav-remove-btn" @click.stop="emit('favoriteRemove', item.id)">
              <el-icon :size="14">
                <StarFilled />
              </el-icon>
            </button>
          </el-tooltip>
          <div class="fav-item-meta">
            <el-tooltip :content="item.conversation_title" placement="top" effect="dark"
              :visible="convTitleTipId === `fav:${item.id}`" :enterable="false" :hide-after="0"
              popper-class="chat-history-dark-tooltip"
              @update:visible="(v: boolean) => { if (!v && convTitleTipId === `fav:${item.id}`) hideConvTitleTip() }">
              <span
                class="fav-conv-title"
                @mouseenter="onConvTitleEnter(`fav:${item.id}`, $event)"
                @mouseleave="onConvTitleLeave(`fav:${item.id}`)"
              >{{ item.conversation_title }}</span>
            </el-tooltip>
            <span v-if="item.conversation_deleted" class="fav-deleted-tag">已删除</span>
          </div>
          <div class="fav-item-content">{{ favContentPreview(item.content) }}</div>
        </div>
      </div>
    </aside>
</template>

<style scoped lang="scss">
.conv-aside {
  width: 234px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--chat-divider-color) !important;
  //background: var(--bg-secondary, var(--bg-primary));
  background: var(--bg-submenu);
  transition: width 0.2s ease;

  &.collapsed {
    width: 0;
    overflow: hidden;
  }
}

.conv-aside-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
}
.btn-new-chat {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}

.btn-collapse {
  width: 33px;
  height: 33px;
  border: 1px solid var(--border-color);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  // background: var(--bg-secondary, rgba(0,0,0,0.03));
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-primary);
  transition: all 0.2s;
  flex-shrink: 0;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);

  &:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  // border: none;
  // border-radius: 6px;
  // background: transparent;
  // cursor: pointer;
  // color: var(--text-secondary);
  // display: flex;
  // align-items: center;
  // justify-content: center;

  // &:hover {
  //   background: var(--bg-hover);
  // }
}

.conv-aside-title {
  padding: 4px 16px 8px;
  font-size: var(--font-size-lg);
  font-weight: 400;
  color: var(--text-secondary);
}

.conv-aside-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.aside-tab-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--text-secondary);
  display: inline-flex;
  padding: 2px;
  border-radius: 6px;
}

.aside-tab-btn:hover {
  background: var(--conv-list-hover-color);
  color: var(--text-primary);
}

.conv-list {
  flex: 1;
  overflow-y: auto;
  padding: 0 8px 12px;
}

.conv-item {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px;
  margin-bottom: 2px;
  border-radius: 6px;
  cursor: pointer;
  font-size: var(--font-size-base);
  font-weight: 400;
  color: var(--text-primary);

  &:hover {
    background: var(--conv-list-hover-color);

  }

  &.active {
    color: var(--text-primary);
    background: var(--conv-list-active-color);
    border: 1px solid var(--conv-list-active-border-color);
    border-radius: 10px;
  }

  .conv-title-group {
    display: inline-flex;
    align-items: center;
    flex: 0 1 auto;
    min-width: 0;
    gap: 6px;
    margin-right: 6px;
  }

  .conv-title {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .conv-tags {
    flex-shrink: 0;
    margin-right: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }



  .conv-agent-tag {
    flex-shrink: 0;
    height: 18px;
    padding: 10px 10px;
    line-height: 16px;
    font-size: 11px;
    /* 统一使用主题色（浅色底 + 主题色文字/描边），不再区分 el-tag 类型色 */
    // background-color: color-mix(in srgb, var(--color-primary) 12%, transparent) !important;
    // border-color: color-mix(in srgb, var(--color-primary) 32%, transparent) !important;
    // color: var(--color-primary) !important;
    background: var(--color-primary) !important;
    color: #fff;
    border: none;
  }

  .conv-more {
    position: relative;
    z-index: 1;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: 4px;
    opacity: 0;

    &:hover {
      background: var(--bg-hover);
    }
  }

  &:hover .conv-more,
  &.active .conv-more {
    opacity: 1;
  }
}

.empty-tip {
  padding: 24px 12px;
  text-align: center;
  font-size: 12px;
  color: var(--text-tertiary);
}

/* 收藏列表在历史栏内的样式 */
.fav-item {
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  position: relative;
}

.fav-remove-btn {
  position: absolute;
  top: 6px;
  right: 6px;
  border: none;
  background: transparent;
  cursor: pointer;
  color: #faad14;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 6px;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s, color 0.15s;
}

.conv-item:hover .fav-remove-btn {
  opacity: 1;
}

.fav-remove-btn:hover {
  background: rgba(244, 63, 94, 0.12);
  color: #f43f5e;
}

.fav-item-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}

.fav-conv-title {
  font-size: 11px;
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fav-deleted-tag {
  font-size: 10px;
  color: #b91c1c;
  background: #fef2f2;
  border-radius: 4px;
  padding: 0 5px;
}

.fav-item-content {
  font-size: 13px;
  color: var(--text-primary);
  width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fav-tip {
  padding: 16px;
  text-align: center;
  color: var(--text-tertiary);
  font-size: 13px;
}

</style>

<script setup lang="ts">
import { ref, watch } from 'vue'
import type { ChatComposerAction } from 'chat-renderer'
import FastClawComposerIcon from '@/integrations/chat-plan/FastClawComposerIcon.vue'
import WebSearchPopover from '@/components/chat/WebSearchPopover.vue'
import McpToolPopover from '@/components/chat/McpToolPopover.vue'

const props = defineProps<{
  action: ChatComposerAction
  active: boolean
  windowListVisible: boolean
  popoverStyle: Record<string, string>
  /** 当前会话 id：供「工具」弹层读写本会话的 Skill / MCP 白名单 */
  conversationId?: string
}>()
const emit = defineEmits<{
  trigger: [action: ChatComposerAction, nativeEvent: MouseEvent]
  toggleWindowList: [anchor: HTMLElement]
  hideSelf: []
  'update:windowListVisible': [visible: boolean]
}>()
const screenshotGroupRef = ref<HTMLElement>()
function toggleWindowList() {
  if (screenshotGroupRef.value) emit('toggleWindowList', screenshotGroupRef.value)
}
watch(() => props.active, active => {
  if (active) return
  if (props.action.id === 'screenshot' && props.windowListVisible) emit('update:windowListVisible', false)
})
</script>

<template>
          <div v-if="action.id === 'screenshot'" ref="screenshotGroupRef" class="screenshot-group">
            <el-tooltip :content="action.label" placement="top">
              <button type="button" class="fastclaw-composer-action fastclaw-composer-action--screenshot"
                :title="action.label" @click.stop="emit('trigger', action, $event)">
                <FastClawComposerIcon name="screenshot" />
              </button>
            </el-tooltip>
            <button type="button" class="hide-win-btn" :title="$t('chat.hideWindowScreenshot')" @click.stop="toggleWindowList">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><polyline points="6 9 12 15 18 9" /></svg>
            </button>
            <Teleport to="body">
              <div v-if="active && windowListVisible" class="hide-window-popover" :style="popoverStyle" @click.stop>
                <div class="hide-win-item" @click="emit('hideSelf')">{{ $t('chat.hideWindowScreenshot') }}</div>
              </div>
            </Teleport>
          </div>
          <WebSearchPopover v-else-if="action.id === 'web-search'" />
          <McpToolPopover v-else-if="action.id === 'mcp'" :conversation-id="conversationId" />
          <button v-else type="button" :class="[
            'fastclaw-composer-action',
            `fastclaw-composer-action--${action.id}`,
            { 'is-active': action.active },
          ]" :disabled="action.disabled" :title="action.label" :aria-pressed="action.active || undefined"
            @click.stop="emit('trigger', action, $event)"><FastClawComposerIcon :name="action.id" /><span class="jcr-sr-only">{{ action.label }}</span></button>
</template>

<style scoped lang="scss">
@use './composerActions';
</style>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useWebSearchStore } from '@/stores/websearch'
import { useAppStore } from '@/stores/app'
import { Check, Settings } from 'lucide-vue-next'
import SvgIcon from '@/components/svgIcon/index.vue'

const ws = useWebSearchStore()
const appStore = useAppStore()
const popRef = ref()

onMounted(() => ws.load())

function pick(id: string, ready: boolean) {
  if (!ready) {
    openSettings()
    return
  }
  ws.provider = id
  if (!ws.enabled) ws.enabled = true
}

function openSettings() {
  ws.settingsTab = 'webSearch'   // 告诉 SettingsView 打开哪个面板
  appStore.showSettings = true   // 真正打开设置弹层
  popRef.value?.hide()           // ★ 关闭这个小弹层（之前不会关）
}
</script>

<template>
  <el-tooltip :content="$t('chat.webSearch')" placement="top">
  <span class="tip-wrap">
  <el-popover ref="popRef" trigger="click" :width="320" placement="top-start" popper-class="ws-pop-wrap">
    <template #reference>
      <button
        class="ws-trigger"
        :class="{ 'is-enabled': ws.enabled }"
      >
        <el-icon :size="18"><svg-icon iconName="internet" /></el-icon>
      </button>
    </template>

    <div class="ws-pop">
      <div class="ws-h">
        <span class="ws-title">{{ $t('chat.webSearch') }}</span>
        <el-switch v-model="ws.enabled" />
      </div>

      <div
        v-for="p in ws.availableProviders"
        :key="p.id"
        class="ws-row"
        :class="{ sel: ws.provider === p.id, disabled: !p.ready }"
        @click="pick(p.id, p.ready)"
      >
        <span class="ws-ico" :style="{ background: p.color }">{{ p.icon }}</span>
        <span class="ws-nm">{{ p.name }}</span>
        <span
          class="ws-badge"
          :class="p.badge === 'free' ? 'free' : (p.ready ? 'ok' : 'key')"
        >
          {{ p.badge === 'free' ? $t('chat.wsFree') : (p.ready ? $t('chat.wsConfigured') : $t('chat.wsNeedKey')) }}
        </span>
        <Check v-if="ws.provider === p.id" :size="16" class="ws-chk" />
      </div>

      <div class="ws-f">
        <span><b>Esc</b> {{ $t('common.close') }}</span>
        <button class="ws-more" @click="openSettings">
          <Settings :size="13" /> {{ $t('chat.wsMoreSettings') }}
        </button>
      </div>
    </div>
  </el-popover>
  </span>
  </el-tooltip>
</template>

<style scoped>
.ws-pop { margin: -4px; }
.ws-h {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 8px 10px; border-bottom: 1px solid var(--border-color);
}
.ws-title { font-size: 14px; font-weight: 600; color: var(--text-primary); }
.ws-row {
  display: flex; align-items: center; gap: 11px;
  padding: 10px 8px; border-radius: 8px; cursor: pointer;
}
.ws-row:hover { background: var(--bg-hover); }
.ws-row.sel { background: rgba(79, 107, 237, 0.07); }
.ws-row.disabled { opacity: 0.55; }
.ws-ico {
  width: 26px; height: 26px; border-radius: 7px;
  display: flex; align-items: center; justify-content: center;
  font-weight: 700; font-size: 13px; color: #fff; flex-shrink: 0;
}
.ws-nm { flex: 1; font-size: 14px; color: var(--text-primary); font-weight: 500; }
.ws-badge { font-size: 11px; padding: 2px 8px; border-radius: 6px; font-weight: 600; }
.ws-badge.free { color: #15a34a; background: #ecfdf3; border: 1px solid #b9eccb; }
.ws-badge.key { color: #ea7517; background: #fff5eb; border: 1px solid #ffdcb8; }
.ws-badge.ok { color: var(--color-primary); background: rgba(79, 107, 237, 0.08); border: 1px solid rgba(79, 107, 237, 0.2); }
.ws-chk { color: var(--color-primary); flex-shrink: 0; }
.ws-f {
  display: flex; align-items: center; gap: 14px;
  padding: 9px 8px 4px; margin-top: 4px;
  border-top: 1px solid var(--border-color);
  font-size: 11px; color: var(--text-secondary);
}
.ws-f b { font-weight: 600; color: var(--text-primary); }
.ws-more {
  margin-left: auto; display: inline-flex; align-items: center; gap: 4px;
  border: none; background: transparent; color: var(--color-primary);
  cursor: pointer; font-size: 12px;
}

.tip-wrap {
  display: inline-flex;
  align-items: center;
}

/* 触发按钮：本组件自带样式（ChatView 的 .btn-icon-only 是 scoped，传不进子组件） */
.ws-trigger {
  border: none;
  background: transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px 8px;
  border-radius: 6px;
  color: color-mix(in srgb, var(--text-secondary) 55%, transparent);
  transition: background 0.15s, color 0.15s;
}
.ws-trigger.is-enabled { color: var(--text-primary); }
.ws-trigger:hover { background: var(--dropdown-hover-color);  }
</style>

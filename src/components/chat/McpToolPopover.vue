<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useMcpChatStore } from '@/stores/mcpChat'
import { useChatToolsStore } from '@/stores/chatTools'
import { Puzzle, Settings, Wrench } from 'lucide-vue-next'

const props = defineProps<{ conversationId?: string }>()

const mcp = useMcpChatStore()
const tools = useChatToolsStore()
const router = useRouter()
const popRef = ref()
const popoverVisible = ref(false)

/** 左右 tab：Skills / MCP 服务 */
const activeTab = ref<'skill' | 'mcp'>('skill')

/** 本地 Skill 列表（含已禁用的，才能用开关重新启用） */
const skills = ref<Array<{ name: string; description?: string; category?: string; enabled: boolean }>>([])
const loadingSkills = ref(false)

async function loadSkills() {
  loadingSkills.value = true
  try {
    const list = await invoke<any[]>('list_local_skills', { category: null, includeDisabled: true })
    skills.value = (Array.isArray(list) ? list : []).map((s) => ({
      name: s.name,
      description: s.description || s.category || '',
      category: s.category || '',
      enabled: s.enabled !== false,
    }))
  } catch {
    skills.value = []
  } finally {
    loadingSkills.value = false
  }
}

async function toggleSkill(name: string) {
  const turningOn = !tools.isSkillEnabled(props.conversationId, name)
  tools.toggleSkill(props.conversationId, name)
  if (!turningOn) return

  // 「Skills 管理」里全局关闭的 skill，工具层（SkillTool / run_skill）会先按全局
  // enabled 过滤掉，光点亮本次对话的开关就成了死开关 —— 开着就是要用，顺手全局启用。
  const row = skills.value.find((s) => s.name === name)
  if (!row || row.enabled) return
  try {
    await invoke('toggle_local_skill', { name, enabled: true })
    row.enabled = true
  } catch {
    // 全局启用失败不回滚本次对话的选择：开关已开，用户想用的就是它
  }
}

async function ensureMcpData() {
  await mcp.load()
  await mcp.ensureServersLoaded()
  await loadSkills()
}

function onVisibleChange(v: boolean) {
  popoverVisible.value = v
  if (v) void ensureMcpData()
}

function open() {
  popoverVisible.value = true
}

function openManage() {
  popoverVisible.value = false
  router.push('/mcp')
}

defineExpose({ open })
</script>

<template>
  <el-tooltip :content="$t('chat.mcpTool')" placement="top">
  <span class="tip-wrap">
  <el-popover
    ref="popRef"
    v-model:visible="popoverVisible"
    trigger="click"
    :width="340"
    placement="top-start"
    popper-class="mcp-pop-wrap"
    @show="onVisibleChange(true)"
    @hide="onVisibleChange(false)"
  >
    <template #reference>
      <button
        class="mcp-trigger"
        :style="{ color: mcp.enabled ? 'var(--text-secondary)' : '' }"
      >
        <el-icon :size="17">
          <Wrench :size="17" />
        </el-icon>
      </button>
    </template>

    <div class="mcp-pop">
      <div class="mcp-h">
        <span class="mcp-title">{{ $t('chat.mcpTool') }}</span>
        <el-switch v-model="mcp.enabled" />
      </div>

      <p class="mcp-desc">{{ $t('chat.mcpToolDesc') }}</p>

      <el-alert
        v-if="mcp.errorCode"
        :title="$t(`chat.mcp${mcp.errorCode === 'managerUnavailable' ? 'ManagerUnavailable' : 'LoadFailed'}`)"
        type="error"
        :closable="false"
        class="mcp-alert"
      />

      <el-tabs v-model="activeTab" class="mcp-tabs">
        <!-- ─── Skills ─── -->
        <el-tab-pane name="skill">
          <template #label>
            <span class="mcp-tab-label">{{ $t('chat.skillsSection') }}<span class="mcp-sec-n">{{ skills.length }}</span></span>
          </template>
          <div v-loading="loadingSkills" class="mcp-list">
            <div v-if="!loadingSkills && skills.length === 0" class="mcp-empty-row">
              {{ $t('chat.skillNoItems') }}
            </div>
            <div v-for="s in skills" :key="`skill-${s.name}`" class="mcp-row">
              <span class="mcp-ico mcp-ico--skill"><Puzzle :size="14" /></span>
              <div class="mcp-info">
                <span class="mcp-nm">{{ s.name }}</span>
                <span class="mcp-sub">{{ s.description }}</span>
              </div>
              <el-switch
                :model-value="tools.isSkillEnabled(props.conversationId, s.name)"
                size="small"
                @change="toggleSkill(s.name)"
                @click.stop
              />
            </div>
          </div>
        </el-tab-pane>

        <!-- ─── MCP 服务 ─── -->
        <el-tab-pane name="mcp">
          <template #label>
            <span class="mcp-tab-label">{{ $t('chat.mcpServersSection') }}<span class="mcp-sec-n">{{ mcp.connectedServers.length }}</span></span>
          </template>
          <div v-loading="mcp.loadingServers" class="mcp-list">
            <div
              v-if="!mcp.loadingServers && !mcp.errorCode && mcp.connectedServers.length === 0"
              class="mcp-empty-row"
            >
              {{ $t('chat.mcpNoServers') }}
            </div>
            <div v-for="s in mcp.connectedServers" :key="`mcp-${s.id}`" class="mcp-row">
              <span class="mcp-ico"><Wrench :size="14" /></span>
              <div class="mcp-info">
                <span class="mcp-nm">{{ s.name }}</span>
                <span class="mcp-sub">{{ $t('chat.mcpToolCount', { n: s.tool_count }) }}</span>
              </div>
              <el-switch
                :model-value="tools.isMcpServerEnabled(props.conversationId, s.id)"
                size="small"
                @change="tools.toggleMcpServer(props.conversationId, s.id)"
                @click.stop
              />
            </div>
          </div>
        </el-tab-pane>
      </el-tabs>

      <div class="mcp-f">
        <button class="mcp-more" @click="openManage">
          <Settings :size="13" /> {{ $t('chat.mcpManage') }}
        </button>
      </div>
    </div>
  </el-popover>
  </span>
  </el-tooltip>
</template>

<style scoped>
.mcp-pop { margin: -4px; }
.mcp-h {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 8px 8px; border-bottom: 1px solid var(--border-color);
}
.mcp-title { font-size: 14px; font-weight: 600; color: var(--text-primary); }
.mcp-desc {
  margin: 8px 8px 4px; font-size: 12px; line-height: 1.5;
  color: var(--text-secondary);
}
.mcp-alert {
  margin: 8px;
}
.mcp-list { min-height: 60px; max-height: 320px; overflow-y: auto; padding: 4px 0; }
.mcp-tabs :deep(.el-tabs__header) { margin: 0 0 2px; }
.mcp-tabs :deep(.el-tabs__nav-wrap::after) { height: 1px; }
.mcp-tabs :deep(.el-tabs__item) { height: 34px; line-height: 34px; font-size: 13px; }
.mcp-tab-label { display: inline-flex; align-items: center; gap: 5px; }
.mcp-sec-n {
  font-weight: 400; font-size: 11px; opacity: .7;
}
.mcp-empty-row {
  padding: 10px 8px; font-size: 12px; color: var(--text-secondary);
}
.mcp-row {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 8px; border-radius: 8px;
}
.mcp-row:hover { background: var(--bg-hover); }
.mcp-ico {
  width: 26px; height: 26px; border-radius: 7px;
  display: flex; align-items: center; justify-content: center;
  background: #f0f5ff; color: #2f54eb; flex-shrink: 0;
}
.mcp-ico--skill { background: #fff0f6; color: #eb2f96; }
.mcp-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.mcp-nm {
  font-size: 13px; font-weight: 500; color: var(--text-primary);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.mcp-sub {
  font-size: 11px; color: var(--text-secondary);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.mcp-f {
  display: flex; align-items: center; gap: 8px;
  padding: 9px 8px 4px; margin-top: 4px;
  border-top: 1px solid var(--border-color);
  font-size: 11px; color: var(--text-secondary);
}
.mcp-more {
  margin-left: auto; display: inline-flex; align-items: center; gap: 4px;
  border: none; background: transparent; color: var(--color-primary);
  cursor: pointer; font-size: 12px;
}

.tip-wrap {
  display: inline-flex;
  align-items: center;
}

.mcp-trigger {
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
.mcp-trigger:hover { background: var(--dropdown-hover-color);  }
</style>

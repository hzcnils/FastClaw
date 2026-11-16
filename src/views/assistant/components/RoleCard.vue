<template>
  <div class="role-card" :class="{ 'is-active': role.isActive }" @click="emit('click', role)">
    <!-- 头像 + 基本信息 -->
    <div class="card-header">
      <div class="role-avatar" :style="avatarStyle">
        <img v-if="role.avatar" :src="role.avatar" alt="" />
        <span v-else class="avatar-letter">{{ avatarLetter }}</span>
      </div>
      <div class="card-info">
        <div class="card-title-row">
          <span class="role-name" :title="role.name">{{ role.name }}</span>
          <span class="status-dot" :class="role.isActive ? 'dot-on' : 'dot-off'" :title="role.isActive ? t('assistant.activated') : t('assistant.inactiveStatus')"></span>
        </div>
        <div class="card-badges">
          <el-tag v-if="role.sourceType === 'square'" size="small" type="info" class="badge-square">{{ t('assistant.fromSquare') }}</el-tag>
          <el-tag v-if="role.isPublished" size="small" type="success" class="badge-pub">{{ t('assistant.published') }}</el-tag>
        </div>
      </div>
    </div>

    <!-- 描述 -->
    <div class="card-desc">{{ role.description || t('assistant.noDescription') }}</div>

    <!-- 工具标签 -->
    <div class="card-tools" v-if="toolTags.length">
      <el-tag v-for="tag in toolTags.slice(0, 3)" :key="tag.label" size="small" :type="tag.type" class="tool-tag">
        {{ tag.label }}
      </el-tag>
      <el-tag v-if="toolTags.length > 3" size="small" type="info">+{{ toolTags.length - 3 }}</el-tag>
    </div>

    <!-- 操作区 -->
    <div class="card-actions" @click.stop>
      <el-button
       class="btn-edit-role"
        size="small"
        :type="role.isActive ? 'success' : 'primary'"
        :disabled="role.isActive"
        @click="onActivate"
      >
        {{ role.isActive ? t('assistant.activated') : t('assistant.activateUse') }}
      </el-button>
      <el-button class="btn-edit-role" size="small" @click="emit('edit', role)">{{ t('assistant.editRole') }}</el-button>
      <el-dropdown trigger="click" @command="onCommand">
        <el-button size="small" text>···</el-button>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item command="publish" v-if="!role.isPublished && role.allowPublish">{{ t('assistant.publish') }}</el-dropdown-item>
            <el-dropdown-item command="unpublish" v-if="role.isPublished">{{ t('assistant.unpublish') }}</el-dropdown-item>
            <el-dropdown-item command="clone">{{ t('assistant.cloneRole') }}</el-dropdown-item>
            <el-dropdown-item command="delete" divided style="color: var(--el-color-danger)">{{ t('assistant.deleteRole') }}</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessageBox } from 'element-plus'
import type { Role } from '@/types/assistant'

const { t } = useI18n()

const props = defineProps<{ role: Role }>()
const emit = defineEmits<{
  click: [role: Role]
  edit: [role: Role]
  activate: [id: string]
  clone: [id: string]
  delete: [id: string]
  publish: [id: string]
  unpublish: [id: string]
}>()

const AVATAR_COLORS = [
  '#4b6bfc', '#10b981', '#f59e0b', '#ef4444',
  '#8b5cf6', '#06b6d4', '#ec4899',
]

const avatarLetter = computed(() => {
  const name = props.role.name || '?'
  return name.charAt(0).toUpperCase()
})

const avatarStyle = computed(() => {
  if (props.role.avatar) return {}
  const code = props.role.id.charCodeAt(0) + props.role.id.charCodeAt(1)
  const color = AVATAR_COLORS[code % AVATAR_COLORS.length]
  return { background: color }
})

const toolTags = computed(() => {
  const tags: { label: string; type: string }[] = []
  const mcps = props.role.mcps ?? []
  const skills = props.role.skills ?? []
  const kbs = props.role.knowledgeBases ?? []
  if (mcps.length) tags.push({ label: `MCP×${mcps.length}`, type: 'warning' })
  if (skills.length) tags.push({ label: `Skills×${skills.length}`, type: 'success' })
  if (kbs.length) tags.push({ label: `KB×${kbs.length}`, type: 'info' })
  return tags
})

async function onActivate() {
  emit('activate', props.role.id)
}

async function onCommand(cmd: string) {
  if (cmd === 'delete') {
    try {
      await ElMessageBox.confirm(t('assistant.deleteConfirmMsg'), t('assistant.deleteConfirmTitle'), {
        confirmButtonText: t('assistant.deleteRole'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
        cancelButtonClass: 'theme-action--secondary',
      })
      emit('delete', props.role.id)
    } catch {}
    return
  }
  if (cmd === 'unpublish') {
    try {
      await ElMessageBox.confirm(t('assistant.unpublishConfirmMsg'), t('assistant.unpublishConfirmTitle'), {
        confirmButtonText: t('assistant.unpublishAction'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
        cancelButtonClass: 'theme-action--secondary',
      })
      emit('unpublish', props.role.id)
    } catch {}
    return
  }
  if (cmd === 'publish') emit('publish', props.role.id)
  if (cmd === 'clone') emit('clone', props.role.id)
}
</script>

<style scoped lang="scss">
.role-card {
  background: var(--card-background-tmd);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 14px;
  cursor: pointer;
  transition: box-shadow 0.2s, transform 0.2s;
  display: flex;
  flex-direction: column;
  gap: 10px;

  &:hover {
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.08);
    transform: translateY(-2px);
  }

  &.is-active {
    border-left: 3px solid var(--el-color-success);
  }
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.role-avatar {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  flex-shrink: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .avatar-letter {
    font-size: 20px;
    font-weight: 700;
    color: #fff;
    line-height: 1;
  }
}

.card-info {
  flex: 1;
  min-width: 0;
}

.card-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.role-name {
  font-size: var(--font-size-base);
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.dot-on { background: var(--el-color-success); }
.dot-off { background: var(--border-color); }

.card-badges {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
 
}

.badge-square, .badge-pub {
   font-size: var(--font-size-sm) ;
}

.card-desc {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.5;
  min-height: 36px;
}

.card-tools {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.tool-tag {
  font-size: 11px;
}

.card-actions {
  display: flex;
  gap: 6px;
  align-items: center;
  padding-top: 10px;
  border-top: 1px solid var(--border-color);
}

:deep(.el-dropdown-menu__item) {
  background: transparent !important;

  &:hover:not(.is-disabled) {
    background: var(--dropdown-hover-color) !important;
    color: var(--text-primary) !important;
  }
  &:focus-visible {
    outline: none;
  }
}

.btn-edit-role {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 12px;
  border-radius: 4px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border-color: var(--btn-primary-border, transparent);
  font-size: var(--font-size-md);
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}
</style>

<style lang="scss">
.el-message-box {
  .theme-action--secondary {
    color: var(--btn-secondary-text) !important;
    background: var(--btn-secondary-bg) !important;
    border-color: var(--btn-secondary-border) !important;

    &:hover {
      background: var(--btn-secondary-hover-bg) !important;
      color: var(--btn-secondary-hover-text) !important;
      border-color: var(--btn-secondary-hover-border) !important;
    }
  }
}
</style>
